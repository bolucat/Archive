// 内部包，实现主控模式功能
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

// 常量定义
const (
	openAPIVersion = "v1"                   // OpenAPI版本
	stateFilePath  = "gob"                  // 实例状态持久化文件路径
	stateFileName  = "nodepass.gob"         // 实例状态持久化文件名
	sseRetryTime   = 3000                   // 重试间隔时间（毫秒）
	apiKeyID       = "********"             // API Key的特殊ID
	tcpingSemLimit = 10                     // TCPing最大并发数
	baseDuration   = 100 * time.Millisecond // 基准持续时间
	maxValueLen    = 256                    // 字符长度限制
)

// Swagger UI HTML模板
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

// Master 实现主控模式功能
type Master struct {
	Common                            // 继承通用功能
	alias         string              // 主控别名
	prefix        string              // API前缀
	version       string              // NP版本
	hostname      string              // 隧道名称
	logLevel      string              // 日志级别
	crtPath       string              // 证书路径
	keyPath       string              // 密钥路径
	instances     sync.Map            // 实例映射表
	server        *http.Server        // HTTP服务器
	tlsConfig     *tls.Config         // TLS配置
	masterURL     *url.URL            // 主控URL
	statePath     string              // 实例状态持久化文件路径
	stateMu       sync.Mutex          // 持久化文件写入互斥锁
	subscribers   sync.Map            // SSE订阅者映射表
	notifyChannel chan *InstanceEvent // 事件通知通道
	tcpingSem     chan struct{}       // TCPing并发控制
	startTime     time.Time           // 启动时间
	periodicDone  chan struct{}       // 定期任务停止信号
}

// Instance 实例信息
type Instance struct {
	ID             string             `json:"id"`        // 实例ID
	Alias          string             `json:"alias"`     // 实例别名
	Type           string             `json:"type"`      // 实例类型
	Status         string             `json:"status"`    // 实例状态
	URL            string             `json:"url"`       // 实例URL
	Config         string             `json:"config"`    // 实例配置
	Restart        bool               `json:"restart"`   // 是否自启动
	Tags           []Tag              `json:"tags"`      // 标签数组
	Mode           int32              `json:"mode"`      // 实例模式
	Ping           int32              `json:"ping"`      // 端内延迟
	Pool           int32              `json:"pool"`      // 池连接数
	TCPS           int32              `json:"tcps"`      // TCP连接数
	UDPS           int32              `json:"udps"`      // UDP连接数
	TCPRX          uint64             `json:"tcprx"`     // TCP接收字节数
	TCPTX          uint64             `json:"tcptx"`     // TCP发送字节数
	UDPRX          uint64             `json:"udprx"`     // UDP接收字节数
	UDPTX          uint64             `json:"udptx"`     // UDP发送字节数
	TCPRXBase      uint64             `json:"-" gob:"-"` // TCP接收字节数基线（不序列化）
	TCPTXBase      uint64             `json:"-" gob:"-"` // TCP发送字节数基线（不序列化）
	UDPRXBase      uint64             `json:"-" gob:"-"` // UDP接收字节数基线（不序列化）
	UDPTXBase      uint64             `json:"-" gob:"-"` // UDP发送字节数基线（不序列化）
	cmd            *exec.Cmd          `json:"-" gob:"-"` // 命令对象（不序列化）
	stopped        chan struct{}      `json:"-" gob:"-"` // 停止信号通道（不序列化）
	deleted        bool               `json:"-" gob:"-"` // 删除标志（不序列化）
	cancelFunc     context.CancelFunc `json:"-" gob:"-"` // 取消函数（不序列化）
	lastCheckPoint time.Time          `json:"-" gob:"-"` // 上次检查点时间（不序列化）
}

// Tag 标签结构体
type Tag struct {
	Key   string `json:"key"`   // 标签键
	Value string `json:"value"` // 标签值
}

// InstanceEvent 实例事件信息
type InstanceEvent struct {
	Type     string    `json:"type"`     // 事件类型：initial, create, update, delete, shutdown, log
	Time     time.Time `json:"time"`     // 事件时间
	Instance *Instance `json:"instance"` // 关联的实例
	Logs     string    `json:"logs"`     // 日志内容
}

// SystemInfo 系统信息结构体
type SystemInfo struct {
	CPU       int    `json:"cpu"`        // CPU使用率 (%)
	MemTotal  uint64 `json:"mem_total"`  // 内存容量字节数
	MemUsed   uint64 `json:"mem_used"`   // 内存已用字节数
	SwapTotal uint64 `json:"swap_total"` // 交换区容量字节数
	SwapUsed  uint64 `json:"swap_used"`  // 交换区已用字节数
	NetRX     uint64 `json:"netrx"`      // 网络接收字节数
	NetTX     uint64 `json:"nettx"`      // 网络发送字节数
	DiskR     uint64 `json:"diskr"`      // 磁盘读取字节数
	DiskW     uint64 `json:"diskw"`      // 磁盘写入字节数
	SysUp     uint64 `json:"sysup"`      // 系统运行时间（秒）
}

// TCPingResult TCPing结果结构体
type TCPingResult struct {
	Target    string  `json:"target"`
	Connected bool    `json:"connected"`
	Latency   int64   `json:"latency"`
	Error     *string `json:"error"`
}

// handleTCPing 处理TCPing请求
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

	// 执行TCPing
	result := m.performTCPing(target)
	writeJSON(w, http.StatusOK, result)
}

// performTCPing 执行单次TCPing
func (m *Master) performTCPing(target string) *TCPingResult {
	result := &TCPingResult{
		Target:    target,
		Connected: false,
		Latency:   0,
		Error:     nil,
	}

	// 并发控制
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

// InstanceLogWriter 实例日志写入器
type InstanceLogWriter struct {
	instanceID string         // 实例ID
	instance   *Instance      // 实例对象
	target     io.Writer      // 目标写入器
	master     *Master        // 主控对象
	checkPoint *regexp.Regexp // 检查点正则表达式
}

// NewInstanceLogWriter 创建新的实例日志写入器
func NewInstanceLogWriter(instanceID string, instance *Instance, target io.Writer, master *Master) *InstanceLogWriter {
	return &InstanceLogWriter{
		instanceID: instanceID,
		instance:   instance,
		target:     target,
		master:     master,
		checkPoint: regexp.MustCompile(`CHECK_POINT\|MODE=(\d+)\|PING=(\d+)ms\|POOL=(\d+)\|TCPS=(\d+)\|UDPS=(\d+)\|TCPRX=(\d+)\|TCPTX=(\d+)\|UDPRX=(\d+)\|UDPTX=(\d+)`),
	}
}

// Write 实现io.Writer接口，处理日志输出并解析统计信息
func (w *InstanceLogWriter) Write(p []byte) (n int, err error) {
	s := string(p)
	scanner := bufio.NewScanner(strings.NewReader(s))

	for scanner.Scan() {
		line := scanner.Text()
		// 解析并处理检查点信息
		if matches := w.checkPoint.FindStringSubmatch(line); len(matches) == 10 {
			// matches[1] = MODE, matches[2] = PING, matches[3] = POOL, matches[4] = TCPS, matches[5] = UDPS, matches[6] = TCPRX, matches[7] = TCPTX, matches[8] = UDPRX, matches[9] = UDPTX
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
			for i, stat := range stats {
				if v, err := strconv.ParseUint(matches[i+6], 10, 64); err == nil {
					*stat = bases[i] + v
				}
			}

			w.instance.lastCheckPoint = time.Now()
			// 仅当实例未被删除时才存储和发送更新事件
			if !w.instance.deleted {
				w.master.instances.Store(w.instanceID, w.instance)
				w.master.sendSSEEvent("update", w.instance)
			}
			// 过滤检查点日志
			continue
		}

		// 输出日志加实例ID
		fmt.Fprintf(w.target, "%s [%s]\n", line, w.instanceID)

		// 仅当实例未被删除时才发送日志事件
		if !w.instance.deleted {
			w.master.sendSSEEvent("log", w.instance, line)
		}
	}

	if err := scanner.Err(); err != nil {
		fmt.Fprintf(w.target, "%s [%s]", s, w.instanceID)
	}
	return len(p), nil
}

// setCorsHeaders 设置跨域响应头
func setCorsHeaders(w http.ResponseWriter) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, PATCH, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization, X-API-Key, Cache-Control")
}

// NewMaster 创建新的主控实例
func NewMaster(parsedURL *url.URL, tlsCode string, tlsConfig *tls.Config, logger *logs.Logger, version string) (*Master, error) {
	// 解析主机地址
	host, err := net.ResolveTCPAddr("tcp", parsedURL.Host)
	if err != nil {
		return nil, fmt.Errorf("newMaster: resolve host failed: %w", err)
	}

	// 获取隧道名称
	var hostname string
	if tlsConfig != nil && tlsConfig.ServerName != "" {
		hostname = tlsConfig.ServerName
	} else {
		hostname = parsedURL.Hostname()
	}

	// 设置API前缀
	prefix := parsedURL.Path
	if prefix == "" || prefix == "/" {
		prefix = "/api"
	} else {
		prefix = strings.TrimRight(prefix, "/")
	}

	// 获取应用程序目录作为状态文件存储位置
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

	// 加载持久化的实例状态
	master.loadState()

	// 启动事件分发器
	go master.startEventDispatcher()

	return master, nil
}

// Run 管理主控生命周期
func (m *Master) Run() {
	m.logger.Info("Master started: %v%v", m.tunnelTCPAddr, m.prefix)

	// 初始化API Key
	apiKey, ok := m.findInstance(apiKeyID)
	if !ok {
		// 如果不存在API Key实例，则创建一个
		apiKey = &Instance{
			ID:  apiKeyID,
			URL: generateAPIKey(),
		}
		m.instances.Store(apiKeyID, apiKey)
		m.saveState()
		m.logger.Info("API Key created: %v", apiKey.URL)
	} else {
		m.logger.Info("API Key loaded: %v", apiKey.URL)
	}

	// 设置HTTP路由
	mux := http.NewServeMux()

	// 创建需要API Key认证的端点
	protectedEndpoints := map[string]http.HandlerFunc{
		fmt.Sprintf("%s/instances", m.prefix):  m.handleInstances,
		fmt.Sprintf("%s/instances/", m.prefix): m.handleInstanceDetail,
		fmt.Sprintf("%s/events", m.prefix):     m.handleSSE,
		fmt.Sprintf("%s/info", m.prefix):       m.handleInfo,
		fmt.Sprintf("%s/tcping", m.prefix):     m.handleTCPing,
	}

	// 创建不需要API Key认证的端点
	publicEndpoints := map[string]http.HandlerFunc{
		fmt.Sprintf("%s/openapi.json", m.prefix): m.handleOpenAPISpec,
		fmt.Sprintf("%s/docs", m.prefix):         m.handleSwaggerUI,
	}

	// API Key 认证中间件
	apiKeyMiddleware := func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			// 设置跨域响应头
			setCorsHeaders(w)
			if r.Method == "OPTIONS" {
				w.WriteHeader(http.StatusOK)
				return
			}

			// 读取API Key，如果存在的话
			apiKeyInstance, keyExists := m.findInstance(apiKeyID)
			if keyExists && apiKeyInstance.URL != "" {
				// 检查请求头中的API Key
				reqAPIKey := r.Header.Get("X-API-Key")
				if reqAPIKey == "" {
					// API Key不存在，返回未授权错误
					httpError(w, "Unauthorized: API key required", http.StatusUnauthorized)
					return
				}

				// 验证API Key
				if reqAPIKey != apiKeyInstance.URL {
					httpError(w, "Unauthorized: Invalid API key", http.StatusUnauthorized)
					return
				}
			}

			// 调用原始处理器
			next(w, r)
		}
	}

	// CORS 中间件
	corsMiddleware := func(next http.HandlerFunc) http.HandlerFunc {
		return func(w http.ResponseWriter, r *http.Request) {
			// 设置跨域响应头
			setCorsHeaders(w)
			if r.Method == "OPTIONS" {
				w.WriteHeader(http.StatusOK)
				return
			}
			next(w, r)
		}
	}

	// 注册受保护的端点
	for path, handler := range protectedEndpoints {
		mux.HandleFunc(path, apiKeyMiddleware(handler))
	}

	// 注册公共端点
	for path, handler := range publicEndpoints {
		mux.HandleFunc(path, corsMiddleware(handler))
	}

	// 创建HTTP服务器
	m.server = &http.Server{
		Addr:      m.tunnelTCPAddr.String(),
		ErrorLog:  m.logger.StdLogger(),
		Handler:   mux,
		TLSConfig: m.tlsConfig,
	}

	// 启动HTTP服务器
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

	// 启动定期任务
	go m.startPeriodicTasks()

	// 处理系统信号
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	<-ctx.Done()
	stop()

	// 优雅关闭
	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := m.Shutdown(shutdownCtx); err != nil {
		m.logger.Error("Master shutdown error: %v", err)
	} else {
		m.logger.Info("Master shutdown complete")
	}
}

// Shutdown 关闭主控
func (m *Master) Shutdown(ctx context.Context) error {
	return m.shutdown(ctx, func() {
		// 通知并关闭SSE连接
		m.shutdownSSEConnections()

		// 停止所有运行中的实例
		var wg sync.WaitGroup
		m.instances.Range(func(key, value any) bool {
			instance := value.(*Instance)
			// 如果实例需要停止，则停止它
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

		// 关闭定期任务
		close(m.periodicDone)

		// 关闭事件通知通道，停止事件分发器
		close(m.notifyChannel)

		// 保存实例状态
		if err := m.saveState(); err != nil {
			m.logger.Error("shutdown: save gob failed: %v", err)
		} else {
			m.logger.Info("Instances saved: %v", m.statePath)
		}

		// 关闭HTTP服务器
		if err := m.server.Shutdown(ctx); err != nil {
			m.logger.Error("shutdown: api shutdown error: %v", err)
		}
	})
}

// startPeriodicTasks 启动所有定期任务
func (m *Master) startPeriodicTasks() {
	go m.startPeriodicBackup()
	go m.startPeriodicCleanup()
	go m.startPeriodicRestart()
}

// startPeriodicBackup 启动定期备份
func (m *Master) startPeriodicBackup() {
	for {
		select {
		case <-time.After(ReloadInterval):
			// 固定备份文件名
			backupPath := fmt.Sprintf("%s.backup", m.statePath)

			if err := m.saveStateToPath(backupPath); err != nil {
				m.logger.Error("startPeriodicBackup: backup state failed: %v", err)
			} else {
				m.logger.Info("State backup saved: %v", backupPath)
			}
		case <-m.periodicDone:
			return
		}
	}
}

// startPeriodicCleanup 启动定期清理重复ID的实例
func (m *Master) startPeriodicCleanup() {
	for {
		select {
		case <-time.After(reportInterval):
			// 收集实例并按ID分组
			idInstances := make(map[string][]*Instance)
			m.instances.Range(func(key, value any) bool {
				if id := key.(string); id != apiKeyID {
					idInstances[id] = append(idInstances[id], value.(*Instance))
				}
				return true
			})

			// 清理重复实例
			for _, instances := range idInstances {
				if len(instances) <= 1 {
					continue
				}

				// 选择保留实例
				keepIdx := 0
				for i, inst := range instances {
					if inst.Status == "running" && instances[keepIdx].Status != "running" {
						keepIdx = i
					}
				}

				// 清理多余实例
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
		case <-m.periodicDone:
			return
		}
	}
}

// startPeriodicRestart 启动定期错误实例重启
func (m *Master) startPeriodicRestart() {
	for {
		select {
		case <-time.After(reportInterval):
			// 收集所有error状态的实例
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

			// 重启所有error状态的实例
			for _, instance := range errorInstances {
				m.stopInstance(instance)
				time.Sleep(baseDuration)
				m.startInstance(instance)
			}
		case <-m.periodicDone:
			return
		}
	}
}

// saveState 保存实例状态到文件
func (m *Master) saveState() error {
	return m.saveStateToPath(m.statePath)
}

// saveStateToPath 保存实例状态到指定路径
func (m *Master) saveStateToPath(filePath string) error {
	if !m.stateMu.TryLock() {
		return nil
	}
	defer m.stateMu.Unlock()

	// 创建持久化数据
	persistentData := make(map[string]*Instance)

	// 从sync.Map转换数据
	m.instances.Range(func(key, value any) bool {
		instance := value.(*Instance)
		persistentData[key.(string)] = instance
		return true
	})

	// 如果没有实例，直接返回
	if len(persistentData) == 0 {
		// 如果状态文件存在，删除它
		if _, err := os.Stat(filePath); err == nil {
			return os.Remove(filePath)
		}
		return nil
	}

	// 确保目录存在
	if err := os.MkdirAll(filepath.Dir(filePath), 0755); err != nil {
		return fmt.Errorf("saveStateToPath: mkdirAll failed: %w", err)
	}

	// 创建临时文件
	tempFile, err := os.CreateTemp(filepath.Dir(filePath), "np-*.tmp")
	if err != nil {
		return fmt.Errorf("saveStateToPath: createTemp failed: %w", err)
	}
	tempPath := tempFile.Name()

	// 删除临时文件的函数，只在错误情况下使用
	removeTemp := func() {
		if _, err := os.Stat(tempPath); err == nil {
			os.Remove(tempPath)
		}
	}

	// 编码数据
	encoder := gob.NewEncoder(tempFile)
	if err := encoder.Encode(persistentData); err != nil {
		tempFile.Close()
		removeTemp()
		return fmt.Errorf("saveStateToPath: encode failed: %w", err)
	}

	// 关闭文件
	if err := tempFile.Close(); err != nil {
		removeTemp()
		return fmt.Errorf("saveStateToPath: close temp file failed: %w", err)
	}

	// 原子地替换文件
	if err := os.Rename(tempPath, filePath); err != nil {
		removeTemp()
		return fmt.Errorf("saveStateToPath: rename temp file failed: %w", err)
	}

	return nil
}

// loadState 从文件加载实例状态
func (m *Master) loadState() {
	// 清理旧的临时文件
	if tmpFiles, _ := filepath.Glob(filepath.Join(filepath.Dir(m.statePath), "np-*.tmp")); tmpFiles != nil {
		for _, f := range tmpFiles {
			os.Remove(f)
		}
	}

	// 检查文件是否存在
	if _, err := os.Stat(m.statePath); os.IsNotExist(err) {
		return
	}

	// 打开文件
	file, err := os.Open(m.statePath)
	if err != nil {
		m.logger.Error("loadState: open file failed: %v", err)
		return
	}
	defer file.Close()

	// 解码数据
	var persistentData map[string]*Instance
	decoder := gob.NewDecoder(file)
	if err := decoder.Decode(&persistentData); err != nil {
		m.logger.Error("loadState: decode file failed: %v", err)
		return
	}

	// 恢复实例
	for id, instance := range persistentData {
		instance.stopped = make(chan struct{})

		// 生成完整配置
		if instance.Config == "" && instance.ID != apiKeyID {
			instance.Config = m.generateConfigURL(instance)
		}

		instance.Tags = nil

		m.instances.Store(id, instance)

		// 处理自启动
		if instance.Restart {
			m.logger.Info("Auto-starting instance: %v [%v]", instance.URL, instance.ID)
			m.startInstance(instance)
		}
	}

	m.logger.Info("Loaded %v instances from %v", len(persistentData), m.statePath)
}

// handleOpenAPISpec 处理OpenAPI规范请求
func (m *Master) handleOpenAPISpec(w http.ResponseWriter, r *http.Request) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(m.generateOpenAPISpec()))
}

// handleSwaggerUI 处理Swagger UI请求
func (m *Master) handleSwaggerUI(w http.ResponseWriter, r *http.Request) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "text/html")
	fmt.Fprintf(w, swaggerUIHTML, m.generateOpenAPISpec())
}

// handleInfo 处理系统信息请求
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

		// 更新主控别名
		if len(reqData.Alias) > maxValueLen {
			httpError(w, fmt.Sprintf("Master alias exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
			return
		}
		m.alias = reqData.Alias

		writeJSON(w, http.StatusOK, m.getMasterInfo())

	default:
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// getMasterInfo 获取完整的主控信息
func (m *Master) getMasterInfo() map[string]any {
	info := map[string]any{
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

// getLinuxSysInfo 获取Linux系统信息
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

	// CPU占用：解析/proc/stat
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
	numCPU := runtime.NumCPU()
	if deltaIdle, deltaTotal := idle2-idle1, total2-total1; deltaTotal > 0 && numCPU > 0 {
		info.CPU = min(int((deltaTotal-deltaIdle)*100/deltaTotal/uint64(numCPU)), 100)
	}

	// RAM占用：解析/proc/meminfo
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

	// 网络I/O：解析/proc/net/dev
	if data, err := os.ReadFile("/proc/net/dev"); err == nil {
		for _, line := range strings.Split(string(data), "\n")[2:] {
			if fields := strings.Fields(line); len(fields) >= 10 {
				ifname := strings.TrimSuffix(fields[0], ":")
				// 排除项
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

	// 磁盘I/O：解析/proc/diskstats
	if data, err := os.ReadFile("/proc/diskstats"); err == nil {
		for line := range strings.SplitSeq(string(data), "\n") {
			if fields := strings.Fields(line); len(fields) >= 14 {
				deviceName := fields[2]
				// 排除项
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

	// 系统运行时间：解析/proc/uptime
	if data, err := os.ReadFile("/proc/uptime"); err == nil {
		if fields := strings.Fields(string(data)); len(fields) > 0 {
			if uptime, err := strconv.ParseFloat(fields[0], 64); err == nil {
				info.SysUp = uint64(uptime)
			}
		}
	}

	return info
}

// handleInstances 处理实例集合请求
func (m *Master) handleInstances(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		// 获取所有实例
		instances := []*Instance{}
		m.instances.Range(func(_, value any) bool {
			instances = append(instances, value.(*Instance))
			return true
		})
		writeJSON(w, http.StatusOK, instances)

	case http.MethodPost:
		// 创建新实例
		var reqData struct {
			URL string `json:"url"`
		}
		if err := json.NewDecoder(r.Body).Decode(&reqData); err != nil || reqData.URL == "" {
			httpError(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		// 解析URL
		parsedURL, err := url.Parse(reqData.URL)
		if err != nil {
			httpError(w, "Invalid URL format", http.StatusBadRequest)
			return
		}

		// 验证实例类型
		instanceType := parsedURL.Scheme
		if instanceType != "client" && instanceType != "server" {
			httpError(w, "Invalid URL scheme", http.StatusBadRequest)
			return
		}

		// 生成实例ID
		id := generateID()
		if _, exists := m.instances.Load(id); exists {
			httpError(w, "Instance ID already exists", http.StatusConflict)
			return
		}

		// 创建实例
		instance := &Instance{
			ID:      id,
			Type:    instanceType,
			URL:     m.enhanceURL(reqData.URL, instanceType),
			Status:  "stopped",
			Restart: true,
			stopped: make(chan struct{}),
		}

		instance.Config = m.generateConfigURL(instance)
		m.instances.Store(id, instance)

		// 启动实例
		go m.startInstance(instance)

		// 保存实例状态
		go func() {
			time.Sleep(baseDuration)
			m.saveState()
		}()
		writeJSON(w, http.StatusCreated, instance)

		// 发送创建事件
		m.sendSSEEvent("create", instance)

	default:
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleInstanceDetail 处理单个实例请求
func (m *Master) handleInstanceDetail(w http.ResponseWriter, r *http.Request) {
	// 获取实例ID
	id := strings.TrimPrefix(r.URL.Path, fmt.Sprintf("%s/instances/", m.prefix))
	if id == "" || id == "/" {
		httpError(w, "Instance ID is required", http.StatusBadRequest)
		return
	}

	// 查找实例
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

// handleGetInstance 处理获取实例信息请求
func (m *Master) handleGetInstance(w http.ResponseWriter, instance *Instance) {
	writeJSON(w, http.StatusOK, instance)
}

// handlePatchInstance 处理更新实例状态请求
func (m *Master) handlePatchInstance(w http.ResponseWriter, r *http.Request, id string, instance *Instance) {
	var reqData struct {
		Alias   string `json:"alias,omitempty"`
		Action  string `json:"action,omitempty"`
		Restart *bool  `json:"restart,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&reqData); err == nil {
		if id == apiKeyID {
			// API Key实例只允许restart操作
			if reqData.Action == "restart" {
				m.regenerateAPIKey(instance)
				// 只有API Key需要在这里发送事件
				m.sendSSEEvent("update", instance)
			}
		} else {
			// 重置流量统计
			if reqData.Action == "reset" {
				instance.TCPRX = 0
				instance.TCPTX = 0
				instance.UDPRX = 0
				instance.UDPTX = 0
				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Traffic stats reset: [%v]", instance.ID)

				// 发送流量统计重置事件
				m.sendSSEEvent("update", instance)
			}

			// 更新自启动设置
			if reqData.Restart != nil && instance.Restart != *reqData.Restart {
				instance.Restart = *reqData.Restart
				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Restart policy updated: %v [%v]", *reqData.Restart, instance.ID)

				// 发送restart策略变更事件
				m.sendSSEEvent("update", instance)
			}

			// 更新实例别名
			if reqData.Alias != "" && instance.Alias != reqData.Alias {
				if len(reqData.Alias) > maxValueLen {
					httpError(w, fmt.Sprintf("Instance alias exceeds maximum length %d", maxValueLen), http.StatusBadRequest)
					return
				}
				instance.Alias = reqData.Alias
				m.instances.Store(id, instance)
				go m.saveState()
				m.logger.Info("Alias updated: %v [%v]", reqData.Alias, instance.ID)

				// 发送别名变更事件
				m.sendSSEEvent("update", instance)
			}

			// 处理当前实例操作
			if reqData.Action != "" && reqData.Action != "reset" {
				m.processInstanceAction(instance, reqData.Action)
			}
		}
	}
	writeJSON(w, http.StatusOK, instance)
}

// handlePutInstance 处理更新实例URL请求
func (m *Master) handlePutInstance(w http.ResponseWriter, r *http.Request, id string, instance *Instance) {
	// API Key实例不允许修改URL
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

	// 解析URL
	parsedURL, err := url.Parse(reqData.URL)
	if err != nil {
		httpError(w, "Invalid URL format", http.StatusBadRequest)
		return
	}

	// 验证实例类型
	instanceType := parsedURL.Scheme
	if instanceType != "client" && instanceType != "server" {
		httpError(w, "Invalid URL scheme", http.StatusBadRequest)
		return
	}

	// 增强URL以便进行重复检测
	enhancedURL := m.enhanceURL(reqData.URL, instanceType)

	// 检查是否与当前实例的URL相同
	if instance.URL == enhancedURL {
		httpError(w, "Instance URL conflict", http.StatusConflict)
		return
	}

	// 如果实例需要停止，先停止它
	if instance.Status != "stopped" {
		m.stopInstance(instance)
		time.Sleep(baseDuration)
	}

	// 更新实例URL和类型
	instance.URL = enhancedURL
	instance.Type = instanceType
	instance.Config = m.generateConfigURL(instance)

	// 更新实例状态
	instance.Status = "stopped"
	m.instances.Store(id, instance)

	// 启动实例
	go m.startInstance(instance)

	// 保存实例状态
	go func() {
		time.Sleep(baseDuration)
		m.saveState()
	}()
	writeJSON(w, http.StatusOK, instance)

	m.logger.Info("Instance URL updated: %v [%v]", instance.URL, instance.ID)
}

// regenerateAPIKey 重新生成API Key
func (m *Master) regenerateAPIKey(instance *Instance) {
	instance.URL = generateAPIKey()
	m.instances.Store(apiKeyID, instance)
	m.logger.Info("API Key regenerated: %v", instance.URL)
	go m.saveState()
	go m.shutdownSSEConnections()
}

// processInstanceAction 处理实例操作
func (m *Master) processInstanceAction(instance *Instance, action string) {
	switch action {
	case "start":
		if instance.Status != "running" {
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

// handleDeleteInstance 处理删除实例请求
func (m *Master) handleDeleteInstance(w http.ResponseWriter, id string, instance *Instance) {
	// API Key实例不允许删除
	if id == apiKeyID {
		httpError(w, "Forbidden: API Key", http.StatusForbidden)
		return
	}

	// 标记实例为已删除
	instance.deleted = true
	m.instances.Store(id, instance)

	if instance.Status != "stopped" {
		m.stopInstance(instance)
	}
	m.instances.Delete(id)
	// 删除实例后保存状态
	go m.saveState()
	w.WriteHeader(http.StatusNoContent)

	// 发送删除事件
	m.sendSSEEvent("delete", instance)
}

// handleSSE 处理SSE连接请求
func (m *Master) handleSSE(w http.ResponseWriter, r *http.Request) {
	// 验证是否为GET请求
	if r.Method != http.MethodGet {
		httpError(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// 设置SSE相关响应头
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("Access-Control-Allow-Origin", "*")

	// 创建唯一的订阅者ID
	subscriberID := generateID()

	// 创建一个通道用于接收事件
	events := make(chan *InstanceEvent, 10)

	// 注册订阅者
	m.subscribers.Store(subscriberID, events)
	defer m.subscribers.Delete(subscriberID)

	// 发送初始重试间隔
	fmt.Fprintf(w, "retry: %d\n\n", sseRetryTime)

	// 获取当前所有实例并发送初始状态
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

	// 设置客户端连接超时
	ctx, cancel := context.WithCancel(r.Context())
	defer cancel()

	// 客户端连接关闭标志
	connectionClosed := make(chan struct{})

	// 监听客户端连接是否关闭
	go func() {
		<-ctx.Done()
		close(connectionClosed)
		// 从映射表中移除并关闭通道
		if ch, exists := m.subscribers.LoadAndDelete(subscriberID); exists {
			close(ch.(chan *InstanceEvent))
		}
	}()

	// 持续发送事件到客户端
	for {
		select {
		case <-connectionClosed:
			return
		case event, ok := <-events:
			if !ok {
				return
			}

			// 序列化事件数据
			data, err := json.Marshal(event)
			if err != nil {
				m.logger.Error("handleSSE: event marshal error: %v", err)
				continue
			}

			// 发送事件
			fmt.Fprintf(w, "event: instance\ndata: %s\n\n", data)
			w.(http.Flusher).Flush()
		}
	}
}

// sendSSEEvent 发送SSE事件的通用函数
func (m *Master) sendSSEEvent(eventType string, instance *Instance, logs ...string) {
	event := &InstanceEvent{
		Type:     eventType,
		Time:     time.Now(),
		Instance: instance,
	}

	// 如果有日志内容，添加到事件中
	if len(logs) > 0 {
		event.Logs = logs[0]
	}

	// 非阻塞方式发送事件
	select {
	case m.notifyChannel <- event:
	default:
		// 通道已满或关闭，忽略
	}
}

// shutdownSSEConnections 通知并关闭SSE连接
func (m *Master) shutdownSSEConnections() {
	var wg sync.WaitGroup

	// 发送shutdown通知并关闭通道
	m.subscribers.Range(func(key, value any) bool {
		ch := value.(chan *InstanceEvent)
		wg.Add(1)
		go func(subscriberID any, eventChan chan *InstanceEvent) {
			defer wg.Done()
			// 发送shutdown通知
			select {
			case eventChan <- &InstanceEvent{Type: "shutdown", Time: time.Now()}:
			default:
			}
			// 从映射表中移除并关闭通道
			if _, exists := m.subscribers.LoadAndDelete(subscriberID); exists {
				close(eventChan)
			}
		}(key, ch)
		return true
	})

	wg.Wait()
}

// startEventDispatcher 启动事件分发器
func (m *Master) startEventDispatcher() {
	for event := range m.notifyChannel {
		// 向所有订阅者分发事件
		m.subscribers.Range(func(_, value any) bool {
			eventChan := value.(chan *InstanceEvent)
			// 非阻塞方式发送事件
			select {
			case eventChan <- event:
			default:
				// 不可用，忽略
			}
			return true
		})
	}
}

// findInstance 查找实例
func (m *Master) findInstance(id string) (*Instance, bool) {
	value, exists := m.instances.Load(id)
	if !exists {
		return nil, false
	}
	return value.(*Instance), true
}

// startInstance 启动实例
func (m *Master) startInstance(instance *Instance) {
	// 获取最新实例状态
	if value, exists := m.instances.Load(instance.ID); exists {
		instance = value.(*Instance)
		if instance.Status == "running" {
			return
		}
	}

	// 启动前，记录基线
	instance.TCPRXBase = instance.TCPRX
	instance.TCPTXBase = instance.TCPTX
	instance.UDPRXBase = instance.UDPRX
	instance.UDPTXBase = instance.UDPTX

	// 获取可执行文件路径
	execPath, err := os.Executable()
	if err != nil {
		m.logger.Error("startInstance: get path failed: %v [%v]", err, instance.ID)
		instance.Status = "error"
		m.instances.Store(instance.ID, instance)
		m.sendSSEEvent("update", instance)
		return
	}

	// 创建上下文和命令
	ctx, cancel := context.WithCancel(context.Background())
	cmd := exec.CommandContext(ctx, execPath, instance.URL)
	instance.cancelFunc = cancel

	// 设置日志输出
	writer := NewInstanceLogWriter(instance.ID, instance, os.Stdout, m)
	cmd.Stdout, cmd.Stderr = writer, writer

	m.logger.Info("Instance starting: %v [%v]", instance.URL, instance.ID)

	// 启动实例
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

	// 发送启动事件
	m.sendSSEEvent("update", instance)
}

// monitorInstance 监控实例状态
func (m *Master) monitorInstance(instance *Instance, cmd *exec.Cmd) {
	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()

	for {
		select {
		case <-instance.stopped:
			// 实例被显式停止
			return
		case err := <-done:
			// 获取最新的实例状态
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

// stopInstance 停止实例
func (m *Master) stopInstance(instance *Instance) {
	// 如果已经是停止状态，不重复操作
	if instance.Status == "stopped" {
		return
	}

	// 如果没有命令或进程，直接设为已停止
	if instance.cmd == nil || instance.cmd.Process == nil {
		instance.Status = "stopped"
		m.instances.Store(instance.ID, instance)
		m.sendSSEEvent("update", instance)
		return
	}

	// 发送终止信号
	if instance.cmd.Process != nil {
		if runtime.GOOS == "windows" {
			instance.cmd.Process.Signal(os.Interrupt)
		} else {
			instance.cmd.Process.Signal(syscall.SIGTERM)
		}
		time.Sleep(baseDuration)
	}

	// 关闭停止通道
	close(instance.stopped)

	// 取消执行或强制终止
	if instance.cancelFunc != nil {
		instance.cancelFunc()
	} else {
		err := instance.cmd.Process.Kill()
		if err != nil {
			m.logger.Error("stopInstance: instance error: %v [%v]", err, instance.ID)
		}
	}

	m.logger.Info("Instance stopped [%v]", instance.ID)

	// 重置实例状态
	instance.Status = "stopped"
	instance.stopped = make(chan struct{})
	instance.cancelFunc = nil
	m.instances.Store(instance.ID, instance)

	// 保存状态变更
	go m.saveState()

	// 发送停止事件
	m.sendSSEEvent("update", instance)
}

// enhanceURL 增强URL，添加日志级别和TLS配置
func (m *Master) enhanceURL(instanceURL string, instanceType string) string {
	parsedURL, err := url.Parse(instanceURL)
	if err != nil {
		m.logger.Error("enhanceURL: invalid URL format: %v", err)
		return instanceURL
	}

	query := parsedURL.Query()

	// 设置日志级别
	if m.logLevel != "" && query.Get("log") == "" {
		query.Set("log", m.logLevel)
	}

	// 为服务端实例设置TLS配置
	if instanceType == "server" && m.tlsCode != "0" {
		if query.Get("tls") == "" {
			query.Set("tls", m.tlsCode)
		}

		// 为TLS code-2设置证书和密钥
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

// generateConfigURL 生成实例的完整URL
func (m *Master) generateConfigURL(instance *Instance) string {
	parsedURL, err := url.Parse(instance.URL)
	if err != nil {
		m.logger.Error("generateConfigURL: invalid URL format: %v", err)
		return instance.URL
	}

	query := parsedURL.Query()

	// 设置日志级别
	if m.logLevel != "" && query.Get("log") == "" {
		query.Set("log", m.logLevel)
	}

	// 设置TLS配置
	if instance.Type == "server" && m.tlsCode != "0" {
		if query.Get("tls") == "" {
			query.Set("tls", m.tlsCode)
		}

		// 为TLS code-2设置证书和密钥
		if m.tlsCode == "2" {
			if m.crtPath != "" && query.Get("crt") == "" {
				query.Set("crt", m.crtPath)
			}
			if m.keyPath != "" && query.Get("key") == "" {
				query.Set("key", m.keyPath)
			}
		}
	}

	// 根据实例类型设置默认参数
	switch instance.Type {
	case "client":
		// client参数: min, mode, read, rate, slot, proxy
		if query.Get("min") == "" {
			query.Set("min", strconv.Itoa(defaultMinPool))
		}
		if query.Get("mode") == "" {
			query.Set("mode", defaultRunMode)
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
	case "server":
		// server参数: max, mode, read, rate, slot, proxy
		if query.Get("max") == "" {
			query.Set("max", strconv.Itoa(defaultMaxPool))
		}
		if query.Get("mode") == "" {
			query.Set("mode", defaultRunMode)
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
	}

	parsedURL.RawQuery = query.Encode()
	return parsedURL.String()
}

// generateID 生成随机ID
func generateID() string {
	bytes := make([]byte, 4)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

// generateAPIKey 生成API Key
func generateAPIKey() string {
	bytes := make([]byte, 16)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

// httpError 返回HTTP错误
func httpError(w http.ResponseWriter, message string, statusCode int) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(map[string]string{"error": message})
}

// writeJSON 写入JSON响应
func writeJSON(w http.ResponseWriter, statusCode int, data any) {
	setCorsHeaders(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(data)
}

// generateOpenAPISpec 生成OpenAPI规范文档
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
		"properties": {"url": {"type": "string", "description": "Command string(scheme://host:port/host:port)"}}
	  },
	  "UpdateInstanceRequest": {
		"type": "object",
		"properties": {
		  "alias": {"type": "string", "description": "Instance alias"},
		  "action": {"type": "string", "enum": ["start", "stop", "restart", "reset"], "description": "Action for the instance"},
		  "restart": {"type": "boolean", "description": "Instance restart policy"}
		}
	  },
	  "PutInstanceRequest": {
		"type": "object",
		"required": ["url"],
		"properties": {"url": {"type": "string", "description": "New command string(scheme://host:port/host:port)"}}
	  },
	  "MasterInfo": {
		"type": "object",
		"properties": {
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
