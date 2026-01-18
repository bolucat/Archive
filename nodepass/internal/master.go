package internal

import (
	"bufio"
	"context"
	"crypto/rand"
	"crypto/tls"
	"encoding/gob"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/NodePassProject/logs"
)

const (
	openAPIVersion  = "v1"
	stateFilePath   = "gob"
	stateFileName   = "nodepass.gob"
	sseRetryTime    = 3000
	apiKeyID        = "********"
	tcpingSemLimit  = 10
	baseDuration    = 100 * time.Millisecond
	gracefulTimeout = 5 * time.Second
	maxValueLen     = 256
)

const swaggerUIHTML = `<!DOCTYPE html>
<html>
<head>
  <title>NodePass API</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
	window.onload = () => SwaggerUIBundle({
	  spec: %s,
	  dom_id: '#swagger-ui',
	  presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
	  layout: "BaseLayout"
	});
  </script>
</body>
</html>`

type Master struct {
	Common
	mid           string
	alias         string
	prefix        string
	version       string
	hostname      string
	logLevel      string
	crtPath       string
	keyPath       string
	instances     sync.Map
	server        *http.Server
	tlsConfig     *tls.Config
	masterURL     *url.URL
	statePath     string
	stateMu       sync.Mutex
	subscribers   sync.Map
	notifyChannel chan *InstanceEvent
	tcpingSem     chan struct{}
	startTime     time.Time
	periodicDone  chan struct{}
}

type Instance struct {
	ID             string             `json:"id"`
	Alias          string             `json:"alias"`
	Type           string             `json:"type"`
	Status         string             `json:"status"`
	URL            string             `json:"url"`
	Config         string             `json:"config"`
	Restart        bool               `json:"restart"`
	Meta           Meta               `json:"meta"`
	Mode           int32              `json:"mode"`
	Ping           int32              `json:"ping"`
	Pool           int32              `json:"pool"`
	TCPS           int32              `json:"tcps"`
	UDPS           int32              `json:"udps"`
	TCPRX          uint64             `json:"tcprx"`
	TCPTX          uint64             `json:"tcptx"`
	UDPRX          uint64             `json:"udprx"`
	UDPTX          uint64             `json:"udptx"`
	TCPRXBase      uint64             `json:"-" gob:"-"`
	TCPTXBase      uint64             `json:"-" gob:"-"`
	UDPRXBase      uint64             `json:"-" gob:"-"`
	UDPTXBase      uint64             `json:"-" gob:"-"`
	TCPRXReset     uint64             `json:"-" gob:"-"`
	TCPTXReset     uint64             `json:"-" gob:"-"`
	UDPRXReset     uint64             `json:"-" gob:"-"`
	UDPTXReset     uint64             `json:"-" gob:"-"`
	cmd            *exec.Cmd          `json:"-" gob:"-"`
	stopped        chan struct{}      `json:"-" gob:"-"`
	deleted        bool               `json:"-" gob:"-"`
	cancelFunc     context.CancelFunc `json:"-" gob:"-"`
	lastCheckPoint time.Time          `json:"-" gob:"-"`
}

type Meta struct {
	Peer Peer              `json:"peer"`
	Tags map[string]string `json:"tags"`
}

type Peer struct {
	SID   string `json:"sid"`
	Type  string `json:"type"`
	Alias string `json:"alias"`
}

type InstanceEvent struct {
	Type     string    `json:"type"`
	Time     time.Time `json:"time"`
	Instance *Instance `json:"instance"`
	Logs     string    `json:"logs"`
}

type SystemInfo struct {
	CPU       int    `json:"cpu"`
	MemTotal  uint64 `json:"mem_total"`
	MemUsed   uint64 `json:"mem_used"`
	SwapTotal uint64 `json:"swap_total"`
	SwapUsed  uint64 `json:"swap_used"`
	NetRX     uint64 `json:"netrx"`
	NetTX     uint64 `json:"nettx"`
	DiskR     uint64 `json:"diskr"`
	DiskW     uint64 `json:"diskw"`
	SysUp     uint64 `json:"sysup"`
}

type TCPingResult struct {
	Target    string  `json:"target"`
	Connected bool    `json:"connected"`
	Latency   int64   `json:"latency"`
	Error     *string `json:"error"`
}

func (m *Master) handleTCPing(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	target := r.URL.Query().Get("target")
	if target == "" {
		httpError(w, "Target address required", http.StatusBadRequest)
		return
	}

	result := m.performTCPing(target)
	writeJSON(w, http.StatusOK, result)
}

func (m *Master) performTCPing(target string) *TCPingResult {
	result := &TCPingResult{
		Target:    target,
		Connected: false,
		Latency:   0,
		Error:     nil,
	}

	select {
	case m.tcpingSem <- struct{}{}:
		defer func() { <-m.tcpingSem }()
	case <-time.After(time.Second):
		errMsg := "too many requests"
		result.Error = &errMsg
		return result
	}

	start := time.Now()
	conn, err := net.DialTimeout("tcp", target, reportInterval)
	if err != nil {
		errMsg := err.Error()
		result.Error = &errMsg
		return result
	}

	result.Connected = true
	result.Latency = time.Since(start).Milliseconds()
	conn.Close()
	return result
}

type InstanceLogWriter struct {
	instanceID string
	instance   *Instance
	target     io.Writer
	master     *Master
	checkPoint *regexp.Regexp
}

func NewInstanceLogWriter(instanceID string, instance *Instance, target io.Writer, master *Master) *InstanceLogWriter {
	return &InstanceLogWriter{
		instanceID: instanceID,
		instance:   instance,
		target:     target,
		master:     master,
		checkPoint: regexp.MustCompile(`CHECK_POINT\|MODE=(\d+)\|PING=(\d+)ms\|POOL=(\d+)\|TCPS=(\d+)\|UDPS=(\d+)\|TCPRX=(\d+)\|TCPTX=(\d+)\|UDPRX=(\d+)\|UDPTX=(\d+)`),
	}
}

func (w *InstanceLogWriter) Write(p []byte) (n int, err error) {
	s := string(p)
	scanner := bufio.NewScanner(strings.NewReader(s))

	for scanner.Scan() {
		line := scanner.Text()
		if matches := w.checkPoint.FindStringSubmatch(line); len(matches) == 10 {
			if mode, err := strconv.ParseInt(matches[1], 10, 32); err == nil {
				w.instance.Mode = int32(mode)
			}
			if ping, err := strconv.ParseInt(matches[2], 10, 32); err == nil {
				w.instance.Ping = int32(ping)
			}
			if pool, err := strconv.ParseInt(matches[3], 10, 32); err == nil {
				w.instance.Pool = int32(pool)
			}
			if tcps, err := strconv.ParseInt(matches[4], 10, 32); err == nil {
				w.instance.TCPS = int32(tcps)
			}
			if udps, err := strconv.ParseInt(matches[5], 10, 32); err == nil {
				w.instance.UDPS = int32(udps)
			}

			stats := []*uint64{&w.instance.TCPRX, &w.instance.TCPTX, &w.instance.UDPRX, &w.instance.UDPTX}
			bases := []uint64{w.instance.TCPRXBase, w.instance.TCPTXBase, w.instance.UDPRXBase, w.instance.UDPTXBase}
			resets := []*uint64{&w.instance.TCPRXReset, &w.instance.TCPTXReset, &w.instance.UDPRXReset, &w.instance.UDPTXReset}
			for i, stat := range stats {
				if v, err := strconv.ParseUint(matches[i+6], 10, 64); err == nil {
					if v >= *resets[i] {
						*stat = bases[i] + v - *resets[i]
					} else {
						*stat = bases[i] + v
						*resets[i] = 0
					}
				}
			}

			w.instance.lastCheckPoint = time.Now()

			if w.instance.Status == "error" {
				w.instance.Status = "running"
			}

			if !w.instance.deleted {
				w.master.instances.Store(w.instanceID, w.instance)
				w.master.sendSSEEvent("update", w.instance)
			}
			continue
		}

		if w.instance.Status != "error" && !w.instance.deleted &&
			(strings.Contains(line, "Server error:") || strings.Contains(line, "Client error:")) {
			w.instance.Status = "error"
			w.instance.Ping = 0
			w.instance.Pool = 0
			w.instance.TCPS = 0
			w.instance.UDPS = 0
			w.master.instances.Store(w.instanceID, w.instance)
		}

		fmt.Fprintf(w.target, "%s [%s]\n", line, w.instanceID)

		if !w.instance.deleted {
			w.master.sendSSEEvent("log", w.instance, line)
		}
	}

	if err := scanner.Err(); err != nil {
		fmt.Fprintf(w.target, "%s [%s]", s, w.instanceID)
	}
	return len(p), nil
}

func setCorsHeaders(w http.ResponseWriter) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, PATCH, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-API-Key, Cache-Control")
}

func NewMaster(parsedURL *url.URL, tlsCode string, tlsConfig *tls.Config, logger *logs.Logger, version string) (*Master, error) {
	host, err := net.ResolveTCPAddr("tcp", parsedURL.Host)
	if err != nil {
		return nil, fmt.Errorf("newMaster: resolve host failed: %w", err)
	}

	var hostname string
	if tlsConfig != nil && tlsConfig.ServerName != "" {
		hostname = tlsConfig.ServerName
	} else {
		hostname = parsedURL.Hostname()
	}

	prefix := parsedURL.Path
	if prefix == "" || prefix == "/" {
		prefix = "/api"
	} else {
		prefix = strings.TrimRight(prefix, "/")
	}

	execPath, _ := os.Executable()
	baseDir := filepath.Dir(execPath)

	master := &Master{
		Common: Common{
			tlsCode: tlsCode,
			logger:  logger,
		},
		prefix:        fmt.Sprintf("%s/%s", prefix, openAPIVersion),
		version:       version,
		logLevel:      parsedURL.Query().Get("log"),
		crtPath:       parsedURL.Query().Get("crt"),
		keyPath:       parsedURL.Query().Get("key"),
		hostname:      hostname,
		tlsConfig:     tlsConfig,
		masterURL:     parsedURL,
		statePath:     filepath.Join(baseDir, stateFilePath, stateFileName),
		notifyChannel: make(chan *InstanceEvent, semaphoreLimit),
		tcpingSem:     make(chan struct{}, tcpingSemLimit),
		startTime:     time.Now(),
		periodicDone:  make(chan struct{}),
	}
	master.tunnelTCPAddr = host

	master.loadState()

	go master.startEventDispatcher()

	return master, nil
}

func (m *Master) Run() {
	m.logger.Info("Master started: %v%v", m.tunnelTCPAddr, m.prefix)

	apiKey, ok := m.findInstance(apiKeyID)
	if !ok {
		apiKey = &Instance{
			ID:     apiKeyID,
			URL:    generateAPIKey(),
			Config: generateMID(),
			Meta:   Meta{Tags: make(map[string]string)},
		}
		m.instances.Store(apiKeyID, apiKey)
		m.saveState()
		fmt.Printf("%s  \033[32mINFO\033[0m  API Key created: %v\n", time.Now().Format("2006-01-02 15:04:05.000"), apiKey.URL)
	} else {
		m.alias = apiKey.Alias

		if apiKey.Config == "" {
			apiKey.Config = generateMID()
			m.instances.Store(apiKeyID, apiKey)
			m.saveState()
			m.logger.Info("Master ID created: %v", apiKey.Config)
		}
		m.mid = apiKey.Config

		fmt.Printf("%s  \033[32mINFO\033[0m  API Key loaded: %v\n", time.Now().Format("2006-01-02 15:04:05.000"), apiKey.URL)
	}

	mux := http.NewServeMux()

	protectedEndpoints := map[string]http.HandlerFunc{
		fmt.Sprintf("%s/instances", m.prefix):  m.handleInstances,
		fmt.Sprintf("%s/instances/", m.prefix): m.handleInstanceDetail,
		fmt.Sprintf("%s/events", m.prefix):     m.handleSSE,
		fmt.Sprintf("%s/info", m.prefix):       m.handleInfo,
		fmt.Sprintf("%s/tcping", m.prefix):     m.handleTCPing,
	}

	publicEndpoints := map[string]http.HandlerFunc{
		fmt.Sprintf("%s/openapi.json", m.prefix): m.handleOpenAPISpec,
		fmt.Sprintf("%s/docs", m.prefix):         m.handleSwaggerUI,
	}

	apiKeyMiddleware := func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			setCorsHeaders(w)
			if r.Method == "OPTIONS" {
				w.WriteHeader(http.StatusOK)
				return
			}

			apiKeyInstance, keyExists := m.findInstance(apiKeyID)
			if keyExists && apiKeyInstance.URL != "" {
				reqAPIKey := r.Header.Get("X-API-Key")
				if reqAPIKey == "" {
					httpError(w, "Unauthorized: API key required", http.StatusUnauthorized)
					return
				}

				if reqAPIKey != apiKeyInstance.URL {
					httpError(w, "Unauthorized: Invalid API key", http.StatusUnauthorized)
					return
				}
			}

			next(w, r)
		}
	}

	corsMiddleware := func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			setCorsHeaders(w)
			if r.Method == "OPTIONS" {
				w.WriteHeader(http.StatusOK)
				return
			}
			next(w, r)
		}
	}

	for path, handler := range protectedEndpoints {
		mux.HandleFunc(path, apiKeyMiddleware(handler))
	}

	for path, handler := range publicEndpoints {
		mux.HandleFunc(path, corsMiddleware(handler))
	}

	m.server = &http.Server{
		Addr:      m.tunnelTCPAddr.String(),
		ErrorLog:  m.logger.StdLogger(),
		Handler:   mux,
		TLSConfig: m.tlsConfig,
	}

	go func() {
		var err error
		if m.tlsConfig != nil {
			err = m.server.ListenAndServeTLS("", "")
		} else {
			err = m.server.ListenAndServe()
		}
		if err != nil && err != http.ErrServerClosed {
			m.logger.Error("run: listen failed: %v", err)
		}
	}()

	go m.startPeriodicTasks()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	<-ctx.Done()
	stop()

	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := m.Shutdown(shutdownCtx); err != nil {
		m.logger.Error("Master shutdown error: %v", err)
	} else {
		m.logger.Info("Master shutdown complete")
	}
}

func (m *Master) Shutdown(ctx context.Context) error {
	return m.shutdown(ctx, func() {
		m.shutdownSSEConnections()

		var wg sync.WaitGroup
		m.instances.Range(func(key, value any) bool {
			instance := value.(*Instance)
			if instance.Status != "stopped" && instance.cmd != nil && instance.cmd.Process != nil {
				wg.Add(1)
				go func(inst *Instance) {
					defer wg.Done()
					m.stopInstance(inst)
				}(instance)
			}
			return true
		})

		wg.Wait()

		close(m.periodicDone)

		close(m.notifyChannel)

		if err := m.saveState(); err != nil {
			m.logger.Error("shutdown: save gob failed: %v", err)
		} else {
			m.logger.Info("Instances saved: %v", m.statePath)
		}

		if err := m.server.Shutdown(ctx); err != nil {
			m.logger.Error("shutdown: api shutdown error: %v", err)
		}
	})
}

func (m *Master) startPeriodicTasks() {
	ticker := time.NewTicker(ReloadInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			m.performPeriodicBackup()
			m.performPeriodicCleanup()
			m.performPeriodicRestart()
		case <-m.periodicDone:
			ticker.Stop()
			return
		}
	}
}

func (m *Master) performPeriodicBackup() {
	backupPath := fmt.Sprintf("%s.backup", m.statePath)

	if err := m.saveStateToPath(backupPath); err != nil {
		m.logger.Error("performPeriodicBackup: backup state failed: %v", err)
	} else {
		m.logger.Info("State backup saved: %v", backupPath)
	}
}

func (m *Master) performPeriodicCleanup() {
	idInstances := make(map[string][]*Instance)
	m.instances.Range(func(key, value any) bool {
		if id := key.(string); id != apiKeyID {
			idInstances[id] = append(idInstances[id], value.(*Instance))
		}
		return true
	})

	for _, instances := range idInstances {
		if len(instances) <= 1 {
			continue
		}

		keepIdx := 0
		for i, inst := range instances {
			if inst.Status == "running" && instances[keepIdx].Status != "running" {
				keepIdx = i
			}
		}

		for i, inst := range instances {
			if i == keepIdx {
				continue
			}
			inst.deleted = true
			if inst.Status != "stopped" {
				m.stopInstance(inst)
			}
			m.instances.Delete(inst.ID)
		}
	}
}

func (m *Master) performPeriodicRestart() {
	var errorInstances []*Instance
	m.instances.Range(func(key, value any) bool {
		if id := key.(string); id != apiKeyID {
			instance := value.(*Instance)
			if instance.Status == "error" && !instance.deleted {
				errorInstances = append(errorInstances, instance)
			}
		}
		return true
	})

	for _, instance := range errorInstances {
		m.stopInstance(instance)
		time.Sleep(baseDuration)
		m.startInstance(instance)
	}
}

func (m *Master) saveState() error {
	return m.saveStateToPath(m.statePath)
}

func (m *Master) saveStateToPath(filePath string) error {
	if !m.stateMu.TryLock() {
		return nil
	}
	defer m.stateMu.Unlock()

	persistentData := make(map[string]*Instance)

	m.instances.Range(func(key, value any) bool {
		instance := value.(*Instance)
		persistentData[key.(string)] = instance
		return true
	})

	if len(persistentData) == 0 {
		if _, err := os.Stat(filePath); err == nil {
			return os.Remove(filePath)
		}
		return nil
	}

	if err := os.MkdirAll(filepath.Dir(filePath), 0755); err != nil {
		return fmt.Errorf("saveStateToPath: mkdirAll failed: %w", err)
	}

	tempFile, err := os.CreateTemp(filepath.Dir(filePath), "np-*.tmp")
	if err != nil {
		return fmt.Errorf("saveStateToPath: createTemp failed: %w", err)
	}
	tempPath := tempFile.Name()

	removeTemp := func() {
		if _, err := os.Stat(tempPath); err == nil {
			os.Remove(tempPath)
		}
	}

	encoder := gob.NewEncoder(tempFile)
	if err := encoder.Encode(persistentData); err != nil {
		tempFile.Close()
		removeTemp()
		return fmt.Errorf("saveStateToPath: encode failed: %w", err)
	}

	if err := tempFile.Close(); err != nil {
		removeTemp()
		return fmt.Errorf("saveStateToPath: close temp file failed: %w", err)
	}

	if err := os.Rename(tempPath, filePath); err != nil {
		removeTemp()
		return fmt.Errorf("saveStateToPath: rename temp file failed: %w", err)
	}

	return nil
}

func (m *Master) loadState() {
	if tmpFiles, _ := filepath.Glob(filepath.Join(filepath.Dir(m.statePath), "np-*.tmp")); tmpFiles != nil {
		for _, f := range tmpFiles {
			os.Remove(f)
		}
	}

	if _, err := os.Stat(m.statePath); os.IsNotExist(err) {
		return
	}

	file, err := os.Open(m.statePath)
	if err != nil {
		m.logger.Error("loadState: open file failed: %v", err)
		return
	}
	defer file.Close()

	var persistentData map[string]*Instance
	decoder := gob.NewDecoder(file)
	if err := decoder.Decode(&persistentData); err != nil {
		m.logger.Error("loadState: decode file failed: %v", err)
		return
	}

	for id, instance := range persistentData {
		instance.stopped = make(chan struct{})

		if instance.ID != apiKeyID {
			instance.Status = "stopped"
		}

		if instance.Config == "" && instance.ID != apiKeyID {
			instance.Config = m.generateConfigURL(instance)
		}

		if instance.Meta.Tags == nil {
			instance.Meta.Tags = make(map[string]string)
		}

		m.instances.Store(id, instance)

		if instance.Restart {
			m.logger.Info("Auto-starting instance: %v [%v]", instance.URL, instance.ID)
			m.startInstance(instance)
			time.Sleep(baseDuration)
		}
	}

	m.logger.Info("Loaded %v instances from %v", len(persistentData), m.statePath)
}

func (m *Master) handleOpenAPISpec(w http.ResponseWriter, r *http.Request) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(m.generateOpenAPISpec()))
}

func (m *Master) handleSwaggerUI(w http.ResponseWriter, r *http.Request) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "text/html")
	fmt.Fprintf(w, swaggerUIHTML, m.generateOpenAPISpec())
}

func (m *Master) handleInfo(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		writeJSON(w, http.StatusOK, m.getMasterInfo())

	case http.MethodPost:
		var reqData struct {
			Alias string `json:"alias"`
		}
		if err := json.NewDecoder(r.Body).Decode(&reqData); err != nil {
			httpError(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		if len(reqData.Alias) > maxValueLen {
			httpError(w, fmt.Sprintf("Master alias exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
			return
		}
		m.alias = reqData.Alias

		if apiKey, ok := m.findInstance(apiKeyID); ok {
			apiKey.Alias = m.alias
			m.instances.Store(apiKeyID, apiKey)
			go m.saveState()
		}

		writeJSON(w, http.StatusOK, m.getMasterInfo())

	default:
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

func (m *Master) getMasterInfo() map[string]any {
	info := map[string]any{
		"mid":        m.mid,
		"alias":      m.alias,
		"os":         runtime.GOOS,
		"arch":       runtime.GOARCH,
		"cpu":        -1,
		"mem_total":  uint64(0),
		"mem_used":   uint64(0),
		"swap_total": uint64(0),
		"swap_used":  uint64(0),
		"netrx":      uint64(0),
		"nettx":      uint64(0),
		"diskr":      uint64(0),
		"diskw":      uint64(0),
		"sysup":      uint64(0),
		"ver":        m.version,
		"name":       m.hostname,
		"uptime":     uint64(time.Since(m.startTime).Seconds()),
		"log":        m.logLevel,
		"tls":        m.tlsCode,
		"crt":        m.crtPath,
		"key":        m.keyPath,
	}

	if runtime.GOOS == "linux" {
		sysInfo := getLinuxSysInfo()
		info["cpu"] = sysInfo.CPU
		info["mem_total"] = sysInfo.MemTotal
		info["mem_used"] = sysInfo.MemUsed
		info["swap_total"] = sysInfo.SwapTotal
		info["swap_used"] = sysInfo.SwapUsed
		info["netrx"] = sysInfo.NetRX
		info["nettx"] = sysInfo.NetTX
		info["diskr"] = sysInfo.DiskR
		info["diskw"] = sysInfo.DiskW
		info["sysup"] = sysInfo.SysUp
	}

	return info
}

func getLinuxSysInfo() SystemInfo {
	info := SystemInfo{
		CPU:       -1,
		MemTotal:  0,
		MemUsed:   0,
		SwapTotal: 0,
		SwapUsed:  0,
		NetRX:     0,
		NetTX:     0,
		DiskR:     0,
		DiskW:     0,
		SysUp:     0,
	}

	if runtime.GOOS != "linux" {
		return info
	}

	readStat := func() (idle, total uint64) {
		data, err := os.ReadFile("/proc/stat")
		if err != nil {
			return
		}
		for line := range strings.SplitSeq(string(data), "\n") {
			if strings.HasPrefix(line, "cpu ") {
				fields := strings.Fields(line)
				for i, v := range fields[1:] {
					val, _ := strconv.ParseUint(v, 10, 64)
					total += val
					if i == 3 {
						idle = val
					}
				}
				break
			}
		}
		return
	}
	idle1, total1 := readStat()
	time.Sleep(baseDuration)
	idle2, total2 := readStat()
	if deltaIdle, deltaTotal := idle2-idle1, total2-total1; deltaTotal > 0 {
		info.CPU = min(int((deltaTotal-deltaIdle)*100/deltaTotal), 100)
	}

	if data, err := os.ReadFile("/proc/meminfo"); err == nil {
		var memTotal, memAvailable, swapTotal, swapFree uint64
		for line := range strings.SplitSeq(string(data), "\n") {
			if fields := strings.Fields(line); len(fields) >= 2 {
				if val, err := strconv.ParseUint(fields[1], 10, 64); err == nil {
					val *= 1024
					switch fields[0] {
					case "MemTotal:":
						memTotal = val
					case "MemAvailable:":
						memAvailable = val
					case "SwapTotal:":
						swapTotal = val
					case "SwapFree:":
						swapFree = val
					}
				}
			}
		}
		info.MemTotal = memTotal
		info.MemUsed = memTotal - memAvailable
		info.SwapTotal = swapTotal
		info.SwapUsed = swapTotal - swapFree
	}

	if data, err := os.ReadFile("/proc/net/dev"); err == nil {
		for _, line := range strings.Split(string(data), "\n")[2:] {
			if fields := strings.Fields(line); len(fields) >= 10 {
				ifname := strings.TrimSuffix(fields[0], ":")
				if strings.HasPrefix(ifname, "lo") || strings.HasPrefix(ifname, "veth") ||
					strings.HasPrefix(ifname, "docker") || strings.HasPrefix(ifname, "podman") ||
					strings.HasPrefix(ifname, "br-") || strings.HasPrefix(ifname, "virbr") {
					continue
				}
				if val, err := strconv.ParseUint(fields[1], 10, 64); err == nil {
					info.NetRX += val
				}
				if val, err := strconv.ParseUint(fields[9], 10, 64); err == nil {
					info.NetTX += val
				}
			}
		}
	}

	if data, err := os.ReadFile("/proc/diskstats"); err == nil {
		for line := range strings.SplitSeq(string(data), "\n") {
			if fields := strings.Fields(line); len(fields) >= 14 {
				deviceName := fields[2]
				if strings.Contains(deviceName, "loop") || strings.Contains(deviceName, "ram") ||
					strings.HasPrefix(deviceName, "dm-") || strings.HasPrefix(deviceName, "md") {
					continue
				}
				if matched, _ := regexp.MatchString(`\d+$`, deviceName); matched {
					continue
				}
				if val, err := strconv.ParseUint(fields[5], 10, 64); err == nil {
					info.DiskR += val * 512
				}
				if val, err := strconv.ParseUint(fields[9], 10, 64); err == nil {
					info.DiskW += val * 512
				}
			}
		}
	}

	if data, err := os.ReadFile("/proc/uptime"); err == nil {
		if fields := strings.Fields(string(data)); len(fields) > 0 {
			if uptime, err := strconv.ParseFloat(fields[0], 64); err == nil {
				info.SysUp = uint64(uptime)
			}
		}
	}

	return info
}

func (m *Master) handleInstances(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		instances := []*Instance{}
		m.instances.Range(func(_, value any) bool {
			instances = append(instances, value.(*Instance))
			return true
		})
		writeJSON(w, http.StatusOK, instances)

	case http.MethodPost:
		var reqData struct {
			Alias string `json:"alias"`
			URL   string `json:"url"`
		}
		if err := json.NewDecoder(r.Body).Decode(&reqData); err != nil || reqData.URL == "" {
			httpError(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		parsedURL, err := url.Parse(reqData.URL)
		if err != nil {
			httpError(w, "Invalid URL format", http.StatusBadRequest)
			return
		}

		instanceType := parsedURL.Scheme
		if instanceType != "client" && instanceType != "server" {
			httpError(w, "Invalid URL scheme", http.StatusBadRequest)
			return
		}

		id := generateID()
		if _, exists := m.instances.Load(id); exists {
			httpError(w, "Instance ID already exists", http.StatusConflict)
			return
		}

		instance := &Instance{
			ID:      id,
			Alias:   reqData.Alias,
			Type:    instanceType,
			URL:     m.enhanceURL(reqData.URL, instanceType),
			Status:  "stopped",
			Restart: true,
			Meta:    Meta{Tags: make(map[string]string)},
			stopped: make(chan struct{}),
		}

		instance.Config = m.generateConfigURL(instance)
		m.instances.Store(id, instance)

		go m.startInstance(instance)

		go func() {
			time.Sleep(baseDuration)
			m.saveState()
		}()
		writeJSON(w, http.StatusCreated, instance)

		m.sendSSEEvent("create", instance)

	default:
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

func (m *Master) handleInstanceDetail(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, fmt.Sprintf("%s/instances/", m.prefix))
	if id == "" || id == "/" {
		httpError(w, "Instance ID is required", http.StatusBadRequest)
		return
	}

	instance, ok := m.findInstance(id)
	if !ok {
		httpError(w, "Instance not found", http.StatusNotFound)
		return
	}

	switch r.Method {
	case http.MethodGet:
		m.handleGetInstance(w, instance)
	case http.MethodPatch:
		m.handlePatchInstance(w, r, id, instance)
	case http.MethodPut:
		m.handlePutInstance(w, r, id, instance)
	case http.MethodDelete:
		m.handleDeleteInstance(w, id, instance)
	default:
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

func (m *Master) handleGetInstance(w http.ResponseWriter, instance *Instance) {
	writeJSON(w, http.StatusOK, instance)
}

func (m *Master) handlePatchInstance(w http.ResponseWriter, r *http.Request, id string, instance *Instance) {
	var reqData struct {
		Alias   string `json:"alias,omitempty"`
		Action  string `json:"action,omitempty"`
		Restart *bool  `json:"restart,omitempty"`
		Meta    *struct {
			Peer *Peer             `json:"peer,omitempty"`
			Tags map[string]string `json:"tags,omitempty"`
		} `json:"meta,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&reqData); err == nil {
		if id == apiKeyID {
			if reqData.Action == "restart" {
				m.regenerateAPIKey(instance)
				m.sendSSEEvent("update", instance)
			}
		} else {
			if reqData.Alias != "" && instance.Alias != reqData.Alias {
				if len(reqData.Alias) > maxValueLen {
					httpError(w, fmt.Sprintf("Instance alias exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
					return
				}
				instance.Alias = reqData.Alias
				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Alias updated: %v [%v]", reqData.Alias, instance.ID)

				m.sendSSEEvent("update", instance)
			}

			if reqData.Action != "" {
				validActions := map[string]bool{
					"start":   true,
					"stop":    true,
					"restart": true,
					"reset":   true,
				}
				if !validActions[reqData.Action] {
					httpError(w, fmt.Sprintf("Invalid action: %s", reqData.Action), http.StatusBadRequest)
					return
				}

				if reqData.Action == "reset" {
					instance.TCPRXReset = instance.TCPRX - instance.TCPRXBase
					instance.TCPTXReset = instance.TCPTX - instance.TCPTXBase
					instance.UDPRXReset = instance.UDPRX - instance.UDPRXBase
					instance.UDPTXReset = instance.UDPTX - instance.UDPTXBase
					instance.TCPRX = 0
					instance.TCPTX = 0
					instance.UDPRX = 0
					instance.UDPTX = 0
					instance.TCPRXBase = 0
					instance.TCPTXBase = 0
					instance.UDPRXBase = 0
					instance.UDPTXBase = 0
					m.instances.Store(id, instance)
					go m.saveState()
					m.logger.Info("Traffic stats reset: 0 [%v]", instance.ID)

					m.sendSSEEvent("update", instance)
				} else {
					m.processInstanceAction(instance, reqData.Action)
				}
			}

			if reqData.Restart != nil && instance.Restart != *reqData.Restart {
				instance.Restart = *reqData.Restart
				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Restart policy updated: %v [%v]", *reqData.Restart, instance.ID)

				m.sendSSEEvent("update", instance)
			}

			if reqData.Meta != nil {
				if reqData.Meta.Peer != nil {
					if len(reqData.Meta.Peer.SID) > maxValueLen {
						httpError(w, fmt.Sprintf("Meta peer.sid exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
						return
					}
					if len(reqData.Meta.Peer.Type) > maxValueLen {
						httpError(w, fmt.Sprintf("Meta peer.type exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
						return
					}
					if len(reqData.Meta.Peer.Alias) > maxValueLen {
						httpError(w, fmt.Sprintf("Meta peer.alias exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
						return
					}
					instance.Meta.Peer = *reqData.Meta.Peer
				}

				if reqData.Meta.Tags != nil {
					seen := make(map[string]bool)
					for key, value := range reqData.Meta.Tags {
						if len(key) > maxValueLen {
							httpError(w, fmt.Sprintf("Meta tag key exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
							return
						}
						if len(value) > maxValueLen {
							httpError(w, fmt.Sprintf("Meta tag value exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
							return
						}
						if seen[key] {
							httpError(w, fmt.Sprintf("Duplicate meta tag key: %s", key), http.StatusBadRequest)
							return
						}
						seen[key] = true
					}
					instance.Meta.Tags = reqData.Meta.Tags
				}

				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Meta updated [%v]", instance.ID)
				m.sendSSEEvent("update", instance)
			}

		}
	}
	writeJSON(w, http.StatusOK, instance)
}

func (m *Master) handlePutInstance(w http.ResponseWriter, r *http.Request, id string, instance *Instance) {
	if id == apiKeyID {
		httpError(w, "Forbidden: API Key", http.StatusForbidden)
		return
	}

	var reqData struct {
		URL string `json:"url"`
	}
	if err := json.NewDecoder(r.Body).Decode(&reqData); err != nil || reqData.URL == "" {
		httpError(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	parsedURL, err := url.Parse(reqData.URL)
	if err != nil {
		httpError(w, "Invalid URL format", http.StatusBadRequest)
		return
	}

	instanceType := parsedURL.Scheme
	if instanceType != "client" && instanceType != "server" {
		httpError(w, "Invalid URL scheme", http.StatusBadRequest)
		return
	}

	enhancedURL := m.enhanceURL(reqData.URL, instanceType)

	if instance.URL == enhancedURL {
		httpError(w, "Instance URL conflict", http.StatusConflict)
		return
	}

	if instance.Status != "stopped" {
		m.stopInstance(instance)
		time.Sleep(baseDuration)
	}

	instance.URL = enhancedURL
	instance.Type = instanceType
	instance.Config = m.generateConfigURL(instance)

	instance.Status = "stopped"
	m.instances.Store(id, instance)

	go m.startInstance(instance)

	go func() {
		time.Sleep(baseDuration)
		m.saveState()
	}()
	writeJSON(w, http.StatusOK, instance)

	m.logger.Info("Instance URL updated: %v [%v]", instance.URL, instance.ID)
}

func (m *Master) regenerateAPIKey(instance *Instance) {
	instance.URL = generateAPIKey()
	m.instances.Store(apiKeyID, instance)
	fmt.Printf("%s  \033[32mINFO\033[0m  API Key regenerated: %v\n", time.Now().Format("2006-01-02 15:04:05.000"), instance.URL)
	go m.saveState()
	go m.shutdownSSEConnections()
}

func (m *Master) processInstanceAction(instance *Instance, action string) {
	switch action {
	case "start":
		if instance.Status == "stopped" {
			go m.startInstance(instance)
		}
	case "stop":
		if instance.Status != "stopped" {
			go m.stopInstance(instance)
		}
	case "restart":
		go func() {
			m.stopInstance(instance)
			time.Sleep(baseDuration)
			m.startInstance(instance)
		}()
	}
}

func (m *Master) handleDeleteInstance(w http.ResponseWriter, id string, instance *Instance) {
	if id == apiKeyID {
		httpError(w, "Forbidden: API Key", http.StatusForbidden)
		return
	}

	instance.deleted = true
	m.instances.Store(id, instance)

	if instance.Status != "stopped" {
		m.stopInstance(instance)
	}
	m.instances.Delete(id)
	go m.saveState()
	w.WriteHeader(http.StatusNoContent)
	m.sendSSEEvent("delete", instance)
}

func (m *Master) handleSSE(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("Access-Control-Allow-Origin", "*")

	subscriberID := generateID()

	events := make(chan *InstanceEvent, 10)

	m.subscribers.Store(subscriberID, events)
	defer m.subscribers.Delete(subscriberID)

	fmt.Fprintf(w, "retry: %d\n\n", sseRetryTime)

	m.instances.Range(func(_, value any) bool {
		instance := value.(*Instance)
		event := &InstanceEvent{
			Type:     "initial",
			Time:     time.Now(),
			Instance: instance,
		}

		data, err := json.Marshal(event)
		if err == nil {
			fmt.Fprintf(w, "event: instance\ndata: %s\n\n", data)
			w.(http.Flusher).Flush()
		}
		return true
	})

	ctx, cancel := context.WithCancel(r.Context())
	defer cancel()

	connectionClosed := make(chan struct{})

	go func() {
		<-ctx.Done()
		close(connectionClosed)
		if ch, exists := m.subscribers.LoadAndDelete(subscriberID); exists {
			close(ch.(chan *InstanceEvent))
		}
	}()

	for {
		select {
		case <-connectionClosed:
			return
		case event, ok := <-events:
			if !ok {
				return
			}

			data, err := json.Marshal(event)
			if err != nil {
				m.logger.Error("handleSSE: event marshal error: %v", err)
				continue
			}

			fmt.Fprintf(w, "event: instance\ndata: %s\n\n", data)
			w.(http.Flusher).Flush()
		}
	}
}

func (m *Master) sendSSEEvent(eventType string, instance *Instance, logs ...string) {
	event := &InstanceEvent{
		Type:     eventType,
		Time:     time.Now(),
		Instance: instance,
	}

	if len(logs) > 0 {
		event.Logs = logs[0]
	}

	select {
	case m.notifyChannel <- event:
	default:
	}
}

func (m *Master) shutdownSSEConnections() {
	var wg sync.WaitGroup

	m.subscribers.Range(func(key, value any) bool {
		ch := value.(chan *InstanceEvent)
		wg.Add(1)
		go func(subscriberID any, eventChan chan *InstanceEvent) {
			defer wg.Done()
			select {
			case eventChan <- &InstanceEvent{Type: "shutdown", Time: time.Now()}:
			default:
			}
			if _, exists := m.subscribers.LoadAndDelete(subscriberID); exists {
				close(eventChan)
			}
		}(key, ch)
		return true
	})

	wg.Wait()
}

func (m *Master) startEventDispatcher() {
	for event := range m.notifyChannel {
		m.subscribers.Range(func(_, value any) bool {
			eventChan := value.(chan *InstanceEvent)
			select {
			case eventChan <- event:
			default:
			}
			return true
		})
	}
}

func (m *Master) findInstance(id string) (*Instance, bool) {
	value, exists := m.instances.Load(id)
	if !exists {
		return nil, false
	}
	return value.(*Instance), true
}

func (m *Master) startInstance(instance *Instance) {
	if value, exists := m.instances.Load(instance.ID); exists {
		instance = value.(*Instance)
		if instance.Status != "stopped" {
			return
		}
	}

	instance.TCPRXBase = instance.TCPRX
	instance.TCPTXBase = instance.TCPTX
	instance.UDPRXBase = instance.UDPRX
	instance.UDPTXBase = instance.UDPTX

	execPath, err := os.Executable()
	if err != nil {
		m.logger.Error("startInstance: get path failed: %v [%v]", err, instance.ID)
		instance.Status = "error"
		m.instances.Store(instance.ID, instance)
		m.sendSSEEvent("update", instance)
		return
	}

	ctx, cancel := context.WithCancel(context.Background())
	cmd := exec.CommandContext(ctx, execPath, instance.URL)
	instance.cancelFunc = cancel

	writer := NewInstanceLogWriter(instance.ID, instance, os.Stdout, m)
	cmd.Stdout, cmd.Stderr = writer, writer

	m.logger.Info("Instance starting: %v [%v]", instance.URL, instance.ID)

	if err := cmd.Start(); err != nil || cmd.Process == nil || cmd.Process.Pid <= 0 {
		if err != nil {
			m.logger.Error("startInstance: instance error: %v [%v]", err, instance.ID)
		} else {
			m.logger.Error("startInstance: instance start failed [%v]", instance.ID)
		}
		instance.Status = "error"
		m.instances.Store(instance.ID, instance)
		m.sendSSEEvent("update", instance)
		cancel()
		return
	}

	instance.cmd = cmd
	instance.Status = "running"
	go m.monitorInstance(instance, cmd)

	m.instances.Store(instance.ID, instance)

	m.sendSSEEvent("update", instance)
}

func (m *Master) monitorInstance(instance *Instance, cmd *exec.Cmd) {
	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()

	for {
		select {
		case <-instance.stopped:
			return
		case err := <-done:
			if value, exists := m.instances.Load(instance.ID); exists {
				instance = value.(*Instance)
				if instance.Status == "running" {
					if err != nil {
						m.logger.Error("monitorInstance: instance error: %v [%v]", err, instance.ID)
						instance.Status = "error"
					} else {
						instance.Status = "stopped"
					}
					m.instances.Store(instance.ID, instance)
					m.sendSSEEvent("update", instance)
				}
			}
			return
		case <-time.After(reportInterval):
			if !instance.lastCheckPoint.IsZero() && time.Since(instance.lastCheckPoint) > 3*reportInterval {
				instance.Status = "error"
				m.instances.Store(instance.ID, instance)
				m.sendSSEEvent("update", instance)
			}
		}
	}
}

func (m *Master) stopInstance(instance *Instance) {
	if instance.Status == "stopped" {
		return
	}

	if instance.cmd == nil || instance.cmd.Process == nil {
		instance.Status = "stopped"
		m.instances.Store(instance.ID, instance)
		m.sendSSEEvent("update", instance)
		return
	}

	select {
	case <-instance.stopped:
	default:
		close(instance.stopped)
	}

	process := instance.cmd.Process
	if runtime.GOOS == "windows" {
		process.Signal(os.Interrupt)
	} else {
		process.Signal(syscall.SIGTERM)
	}
	if instance.cancelFunc != nil {
		instance.cancelFunc()
	}

	done := make(chan struct{})
	go func() {
		process.Wait()
		close(done)
	}()

	select {
	case <-done:
		m.logger.Info("Instance stopped [%v]", instance.ID)
	case <-time.After(gracefulTimeout):
		process.Kill()
		<-done
		m.logger.Warn("Instance force killed [%v]", instance.ID)
	}

	instance.Status = "stopped"
	instance.stopped = make(chan struct{})
	instance.cancelFunc = nil
	instance.Ping = 0
	instance.Pool = 0
	instance.TCPS = 0
	instance.UDPS = 0
	m.instances.Store(instance.ID, instance)

	go m.saveState()

	m.sendSSEEvent("update", instance)
}

func (m *Master) enhanceURL(instanceURL string, instanceType string) string {
	parsedURL, err := url.Parse(instanceURL)
	if err != nil {
		m.logger.Error("enhanceURL: invalid URL format: %v", err)
		return instanceURL
	}

	query := parsedURL.Query()

	if m.logLevel != "" && query.Get("log") == "" {
		query.Set("log", m.logLevel)
	}

	if instanceType == "server" && m.tlsCode != "0" {
		if query.Get("tls") == "" {
			query.Set("tls", m.tlsCode)
		}

		if m.tlsCode == "2" {
			if m.crtPath != "" && query.Get("crt") == "" {
				query.Set("crt", m.crtPath)
			}
			if m.keyPath != "" && query.Get("key") == "" {
				query.Set("key", m.keyPath)
			}
		}
	}

	parsedURL.RawQuery = query.Encode()
	return parsedURL.String()
}

func (m *Master) generateConfigURL(instance *Instance) string {
	parsedURL, err := url.Parse(instance.URL)
	if err != nil {
		m.logger.Error("generateConfigURL: invalid URL format: %v", err)
		return instance.URL
	}

	query := parsedURL.Query()

	if m.logLevel != "" && query.Get("log") == "" {
		query.Set("log", m.logLevel)
	}

	if instance.Type == "server" && m.tlsCode != "0" {
		if query.Get("tls") == "" {
			query.Set("tls", m.tlsCode)
		}

		if m.tlsCode == "2" {
			if m.crtPath != "" && query.Get("crt") == "" {
				query.Set("crt", m.crtPath)
			}
			if m.keyPath != "" && query.Get("key") == "" {
				query.Set("key", m.keyPath)
			}
		}
	}

	switch instance.Type {
	case "client":
		if query.Get("dns") == "" {
			query.Set("dns", defaultDNSTTL.String())
		}
		if query.Get("sni") == "" {
			query.Set("sni", defaultServerName)
		}
		if query.Get("lbs") == "" {
			query.Set("lbs", defaultLBStrategy)
		}
		if query.Get("min") == "" {
			query.Set("min", strconv.Itoa(defaultMinPool))
		}
		if query.Get("mode") == "" {
			query.Set("mode", defaultRunMode)
		}
		if query.Get("dial") == "" {
			query.Set("dial", defaultDialerIP)
		}
		if query.Get("read") == "" {
			query.Set("read", defaultReadTimeout.String())
		}
		if query.Get("rate") == "" {
			query.Set("rate", strconv.Itoa(defaultRateLimit))
		}
		if query.Get("slot") == "" {
			query.Set("slot", strconv.Itoa(defaultSlotLimit))
		}
		if query.Get("proxy") == "" {
			query.Set("proxy", defaultProxyProtocol)
		}
		if query.Get("block") == "" {
			query.Set("block", defaultBlockProtocol)
		}
		if query.Get("notcp") == "" {
			query.Set("notcp", defaultTCPStrategy)
		}
		if query.Get("noudp") == "" {
			query.Set("noudp", defaultUDPStrategy)
		}
	case "server":
		if query.Get("dns") == "" {
			query.Set("dns", defaultDNSTTL.String())
		}
		if query.Get("lbs") == "" {
			query.Set("lbs", defaultLBStrategy)
		}
		if query.Get("max") == "" {
			query.Set("max", strconv.Itoa(defaultMaxPool))
		}
		if query.Get("mode") == "" {
			query.Set("mode", defaultRunMode)
		}
		if query.Get("type") == "" {
			query.Set("type", defaultPoolType)
		}
		if query.Get("dial") == "" {
			query.Set("dial", defaultDialerIP)
		}
		if query.Get("read") == "" {
			query.Set("read", defaultReadTimeout.String())
		}
		if query.Get("rate") == "" {
			query.Set("rate", strconv.Itoa(defaultRateLimit))
		}
		if query.Get("slot") == "" {
			query.Set("slot", strconv.Itoa(defaultSlotLimit))
		}
		if query.Get("proxy") == "" {
			query.Set("proxy", defaultProxyProtocol)
		}
		if query.Get("block") == "" {
			query.Set("block", defaultBlockProtocol)
		}
		if query.Get("notcp") == "" {
			query.Set("notcp", defaultTCPStrategy)
		}
		if query.Get("noudp") == "" {
			query.Set("noudp", defaultUDPStrategy)
		}
	}

	parsedURL.RawQuery = query.Encode()
	return parsedURL.String()
}

func generateID() string {
	bytes := make([]byte, 4)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

func generateMID() string {
	bytes := make([]byte, 8)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

func generateAPIKey() string {
	bytes := make([]byte, 16)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

func httpError(w http.ResponseWriter, message string, statusCode int) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(map[string]string{"error": message})
}

func writeJSON(w http.ResponseWriter, statusCode int, data any) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(data)
}

func (m *Master) generateOpenAPISpec() string {
	return fmt.Sprintf(`{
  "openapi": "3.1.1",
  "info": {
	"title": "NodePass API",
	"description": "API for managing NodePass server and client instances",
	"version": "%s"
  },
  "servers": [{"url": "%s"}],
  "security": [{"ApiKeyAuth": []}],
  "paths": {
	"/instances": {
	  "get": {
		"summary": "List all instances",
		"security": [{"ApiKeyAuth": []}],
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Instance"}}}}},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"}
		}
	  },
	  "post": {
		"summary": "Create a new instance",
		"security": [{"ApiKeyAuth": []}],
		"requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateInstanceRequest"}}}},
		"responses": {
		  "201": {"description": "Created", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Instance"}}}},
		  "400": {"description": "Invalid input"},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"},
		  "409": {"description": "Instance ID already exists"}
		}
	  }
	},
	"/instances/{id}": {
	  "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}],
	  "get": {
		"summary": "Get instance details",
		"security": [{"ApiKeyAuth": []}],
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Instance"}}}},
		  "400": {"description": "Instance ID required"},
		  "401": {"description": "Unauthorized"},
		  "404": {"description": "Not found"},
		  "405": {"description": "Method not allowed"}
		}
	  },
	  "patch": {
		"summary": "Update instance",
		"security": [{"ApiKeyAuth": []}],
		"requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/UpdateInstanceRequest"}}}},
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Instance"}}}},
		  "400": {"description": "Instance ID required or invalid input"},
		  "401": {"description": "Unauthorized"},
		  "404": {"description": "Not found"},
		  "405": {"description": "Method not allowed"}
		}
	  },
	  "put": {
		"summary": "Update instance URL",
		"security": [{"ApiKeyAuth": []}],
		"requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/PutInstanceRequest"}}}},
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Instance"}}}},
		  "400": {"description": "Instance ID required or invalid input"},
		  "401": {"description": "Unauthorized"},
		  "403": {"description": "Forbidden"},
		  "404": {"description": "Not found"},
		  "405": {"description": "Method not allowed"},
		  "409": {"description": "Instance URL conflict"}
		}
	  },
	  "delete": {
		"summary": "Delete instance",
		"security": [{"ApiKeyAuth": []}],
		"responses": {
		  "204": {"description": "Deleted"},
		  "400": {"description": "Instance ID required"},
		  "401": {"description": "Unauthorized"},
		  "403": {"description": "Forbidden"},
		  "404": {"description": "Not found"},
		  "405": {"description": "Method not allowed"}
		}
	  }
	},
	"/events": {
	  "get": {
		"summary": "Subscribe to instance events",
		"security": [{"ApiKeyAuth": []}],
		"responses": {
		  "200": {"description": "Success", "content": {"text/event-stream": {}}},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"}
		}
	  }
	},
	"/info": {
	  "get": {
		"summary": "Get master information",
		"security": [{"ApiKeyAuth": []}],
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/MasterInfo"}}}},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"}
		}
	  },
	  "post": {
		"summary": "Update master alias",
		"security": [{"ApiKeyAuth": []}],
		"requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/UpdateMasterAliasRequest"}}}},
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/MasterInfo"}}}},
		  "400": {"description": "Invalid input"},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"}
		}
	  }
	},
	"/tcping": {
	  "get": {
		"summary": "TCP connectivity test",
		"security": [{"ApiKeyAuth": []}],
		"parameters": [
		  {
			"name": "target",
			"in": "query",
			"required": true,
			"schema": {"type": "string"},
			"description": "Target address in format host:port"
		  }
		],
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/TCPingResult"}}}},
		  "400": {"description": "Target address required"},
		  "401": {"description": "Unauthorized"},
		  "405": {"description": "Method not allowed"}
		}
	  }
	},
	"/openapi.json": {
	  "get": {
		"summary": "Get OpenAPI specification",
		"responses": {
		  "200": {"description": "Success", "content": {"application/json": {}}}
		}
	  }
	},
	"/docs": {
	  "get": {
		"summary": "Get Swagger UI",
		"responses": {
		  "200": {"description": "Success", "content": {"text/html": {}}}
		}
	  }
	}
  },
  "components": {
   "securitySchemes": {
	 "ApiKeyAuth": {
	"type": "apiKey",
	"in": "header",
	"name": "X-API-Key",
	"description": "API Key for authentication"
	 }
   },
   "schemas": {
	 "Instance": {
	"type": "object",
	"properties": {
	  "id": {"type": "string", "description": "Unique identifier"},
	  "alias": {"type": "string", "description": "Instance alias"},
	  "type": {"type": "string", "enum": ["client", "server"], "description": "Type of instance"},
	  "status": {"type": "string", "enum": ["running", "stopped", "error"], "description": "Instance status"},
	  "url": {"type": "string", "description": "Command string or API Key"},
	  "config": {"type": "string", "description": "Instance configuration URL"},
	  "restart": {"type": "boolean", "description": "Restart policy"},
	  "meta": {"$ref": "#/components/schemas/Meta"},
	  "mode": {"type": "integer", "description": "Instance mode"},
	  "ping": {"type": "integer", "description": "TCPing latency"},
	  "pool": {"type": "integer", "description": "Pool active count"},
	  "tcps": {"type": "integer", "description": "TCP connection count"},
	  "udps": {"type": "integer", "description": "UDP connection count"},
	  "tcprx": {"type": "integer", "description": "TCP received bytes"},
	  "tcptx": {"type": "integer", "description": "TCP transmitted bytes"},
	  "udprx": {"type": "integer", "description": "UDP received bytes"},
	  "udptx": {"type": "integer", "description": "UDP transmitted bytes"}
	}
	 },
	  "CreateInstanceRequest": {
		"type": "object",
		"required": ["url"],
		"properties": {
		  "alias": {"type": "string", "description": "Instance alias"},
		  "url": {"type": "string", "description": "Command string(scheme://host:port/host:port)"}
		}
	  },
	  "UpdateInstanceRequest": {
		"type": "object",
		"properties": {
		  "alias": {"type": "string", "description": "Instance alias"},
		  "action": {"type": "string", "enum": ["start", "stop", "restart", "reset"], "description": "Action for the instance"},
		  "restart": {"type": "boolean", "description": "Instance restart policy"},
		  "meta": {"$ref": "#/components/schemas/Meta"}
		}
	  },
	  "PutInstanceRequest": {
		"type": "object",
		"required": ["url"],
		"properties": {"url": {"type": "string", "description": "New command string(scheme://host:port/host:port)"}}
	  },
	  "Meta": {
		"type": "object",
		"properties": {
		  "peer": {"$ref": "#/components/schemas/Peer"},
		  "tags": {"type": "object", "additionalProperties": {"type": "string"}, "description": "Key-value tags"}
		}
	  },
	  "Peer": {
		"type": "object",
		"properties": {
		  "sid": {"type": "string", "description": "Service ID"},
		  "type": {"type": "string", "description": "Service type"},
		  "alias": {"type": "string", "description": "Service alias"}
		}
	  },
	  "MasterInfo": {
		"type": "object",
		"properties": {
		  "mid": {"type": "string", "description": "Master ID"},
		  "alias": {"type": "string", "description": "Master alias"},
		  "os": {"type": "string", "description": "Operating system"},
		  "arch": {"type": "string", "description": "System architecture"},
		  "cpu": {"type": "integer", "description": "CPU usage percentage"},
		  "mem_total": {"type": "integer", "format": "int64", "description": "Total memory in bytes"},
		  "mem_used": {"type": "integer", "format": "int64", "description": "Used memory in bytes"},
		  "swap_total": {"type": "integer", "format": "int64", "description": "Total swap space in bytes"},
		  "swap_used": {"type": "integer", "format": "int64", "description": "Used swap space in bytes"},
		  "netrx": {"type": "integer", "format": "int64", "description": "Network received bytes"},
		  "nettx": {"type": "integer", "format": "int64", "description": "Network transmitted bytes"},
		  "diskr": {"type": "integer", "format": "int64", "description": "Disk read bytes"},
		  "diskw": {"type": "integer", "format": "int64", "description": "Disk write bytes"},
		  "sysup": {"type": "integer", "format": "int64", "description": "System uptime in seconds"},
		  "ver": {"type": "string", "description": "NodePass version"},
		  "name": {"type": "string", "description": "Hostname"},
		  "uptime": {"type": "integer", "format": "int64", "description": "API uptime in seconds"},
		  "log": {"type": "string", "description": "Log level"},
		  "tls": {"type": "string", "description": "TLS code"},
		  "crt": {"type": "string", "description": "Certificate path"},
		  "key": {"type": "string", "description": "Private key path"}
		}
	  },
	  "UpdateMasterAliasRequest": {
		"type": "object",
		"required": ["alias"],
		"properties": {"alias": {"type": "string", "description": "Master alias"}}
	  },
	  "TCPingResult": {
		"type": "object",
		"properties": {
		  "target": {"type": "string", "description": "Target address"},
		  "connected": {"type": "boolean", "description": "Is connected"},
		  "latency": {"type": "integer", "format": "int64", "description": "Latency in milliseconds"},
		  "error": {"type": "string", "nullable": true, "description": "Error message"}
		}
	  }
	}
  }
}`, openAPIVersion, m.prefix)
}
