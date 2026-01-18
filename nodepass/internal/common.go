package internal

import (
	"bufio"
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"hash/fnv"
	"io"
	"math/big"
	"net"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/NodePassProject/conn"
	"github.com/NodePassProject/logs"
)

type Common struct {
	parsedURL        *url.URL
	logger           *logs.Logger
	dnsCacheTTL      time.Duration
	dnsCacheEntries  sync.Map
	tlsCode          string
	tlsConfig        *tls.Config
	coreType         string
	runMode          string
	poolType         string
	dataFlow         string
	serverName       string
	serverPort       string
	clientIP         string
	dialerIP         string
	dialerFallback   uint32
	tunnelKey        string
	tunnelAddr       string
	tunnelTCPAddr    *net.TCPAddr
	tunnelUDPAddr    *net.UDPAddr
	targetAddrs      []string
	targetTCPAddrs   []*net.TCPAddr
	targetUDPAddrs   []*net.UDPAddr
	targetIdx        uint64
	lastFallback     uint64
	bestLatency      int32
	lbStrategy       string
	targetListener   *net.TCPListener
	tunnelListener   net.Listener
	controlConn      net.Conn
	tunnelUDPConn    *conn.StatConn
	targetUDPConn    *conn.StatConn
	targetUDPSession sync.Map
	tunnelPool       TransportPool
	minPoolCapacity  int
	maxPoolCapacity  int
	proxyProtocol    string
	blockProtocol    string
	blockSOCKS       bool
	blockHTTP        bool
	blockTLS         bool
	disableTCP       string
	disableUDP       string
	rateLimit        int
	rateLimiter      *conn.RateLimiter
	readTimeout      time.Duration
	bufReader        *bufio.Reader
	tcpBufferPool    *sync.Pool
	udpBufferPool    *sync.Pool
	signalChan       chan Signal
	writeChan        chan []byte
	verifyChan       chan struct{}
	handshakeStart   time.Time
	checkPoint       time.Time
	slotLimit        int32
	tcpSlot          int32
	udpSlot          int32
	tcpRX            uint64
	tcpTX            uint64
	udpRX            uint64
	udpTX            uint64
	ctx              context.Context
	cancel           context.CancelFunc
}

type dnsCacheEntry struct {
	tcpAddr   *net.TCPAddr
	udpAddr   *net.UDPAddr
	expiredAt time.Time
}

type readerConn struct {
	net.Conn
	reader io.Reader
}

func (rc *readerConn) Read(b []byte) (int, error) {
	return rc.reader.Read(b)
}

type TransportPool interface {
	IncomingGet(timeout time.Duration) (string, net.Conn, error)
	OutgoingGet(id string, timeout time.Duration) (net.Conn, error)
	Flush()
	Close()
	Ready() bool
	Active() int
	Capacity() int
	Interval() time.Duration
	AddError()
	ErrorCount() int
	ResetError()
}

type Signal struct {
	ActionType  string `json:"action"`
	RemoteAddr  string `json:"remote,omitempty"`
	PoolConnID  string `json:"id,omitempty"`
	Fingerprint string `json:"fp,omitempty"`
}

var (
	semaphoreLimit   = getEnvAsInt("NP_SEMAPHORE_LIMIT", 65536)
	tcpDataBufSize   = getEnvAsInt("NP_TCP_DATA_BUF_SIZE", 16384)
	udpDataBufSize   = getEnvAsInt("NP_UDP_DATA_BUF_SIZE", 16384)
	handshakeTimeout = getEnvAsDuration("NP_HANDSHAKE_TIMEOUT", 5*time.Second)
	tcpDialTimeout   = getEnvAsDuration("NP_TCP_DIAL_TIMEOUT", 5*time.Second)
	udpDialTimeout   = getEnvAsDuration("NP_UDP_DIAL_TIMEOUT", 5*time.Second)
	udpReadTimeout   = getEnvAsDuration("NP_UDP_READ_TIMEOUT", 30*time.Second)
	poolGetTimeout   = getEnvAsDuration("NP_POOL_GET_TIMEOUT", 5*time.Second)
	minPoolInterval  = getEnvAsDuration("NP_MIN_POOL_INTERVAL", 100*time.Millisecond)
	maxPoolInterval  = getEnvAsDuration("NP_MAX_POOL_INTERVAL", 1*time.Second)
	reportInterval   = getEnvAsDuration("NP_REPORT_INTERVAL", 5*time.Second)
	fallbackInterval = getEnvAsDuration("NP_FALLBACK_INTERVAL", 5*time.Minute)
	serviceCooldown  = getEnvAsDuration("NP_SERVICE_COOLDOWN", 3*time.Second)
	shutdownTimeout  = getEnvAsDuration("NP_SHUTDOWN_TIMEOUT", 5*time.Second)
	ReloadInterval   = getEnvAsDuration("NP_RELOAD_INTERVAL", 1*time.Hour)
)

const (
	contextCheckInterval = 50 * time.Millisecond
	defaultDNSTTL        = 5 * time.Minute
	defaultMinPool       = 64
	defaultMaxPool       = 1024
	defaultServerName    = "none"
	defaultLBStrategy    = "0"
	defaultRunMode       = "0"
	defaultPoolType      = "0"
	defaultDialerIP      = "auto"
	defaultReadTimeout   = 0 * time.Second
	defaultRateLimit     = 0
	defaultSlotLimit     = 65536
	defaultProxyProtocol = "0"
	defaultBlockProtocol = "0"
	defaultTCPStrategy   = "0"
	defaultUDPStrategy   = "0"
)

func (c *Common) getTCPBuffer() []byte {
	buf := c.tcpBufferPool.Get().(*[]byte)
	return (*buf)[:tcpDataBufSize]
}

func (c *Common) putTCPBuffer(buf []byte) {
	if buf != nil && cap(buf) >= tcpDataBufSize {
		c.tcpBufferPool.Put(&buf)
	}
}

func (c *Common) getUDPBuffer() []byte {
	buf := c.udpBufferPool.Get().(*[]byte)
	return (*buf)[:udpDataBufSize]
}

func (c *Common) putUDPBuffer(buf []byte) {
	if buf != nil && cap(buf) >= udpDataBufSize {
		c.udpBufferPool.Put(&buf)
	}
}

func (c *Common) tryAcquireSlot(isUDP bool) bool {
	if c.slotLimit == 0 {
		return true
	}

	currentTotal := atomic.LoadInt32(&c.tcpSlot) + atomic.LoadInt32(&c.udpSlot)
	if currentTotal >= c.slotLimit {
		return false
	}

	if isUDP {
		atomic.AddInt32(&c.udpSlot, 1)
	} else {
		atomic.AddInt32(&c.tcpSlot, 1)
	}
	return true
}

func (c *Common) releaseSlot(isUDP bool) {
	if c.slotLimit == 0 {
		return
	}

	if isUDP {
		if current := atomic.LoadInt32(&c.udpSlot); current > 0 {
			atomic.AddInt32(&c.udpSlot, -1)
		}
	} else {
		if current := atomic.LoadInt32(&c.tcpSlot); current > 0 {
			atomic.AddInt32(&c.tcpSlot, -1)
		}
	}
}

func getEnvAsInt(name string, defaultValue int) int {
	if valueStr, exists := os.LookupEnv(name); exists {
		if value, err := strconv.Atoi(valueStr); err == nil && value >= 0 {
			return value
		}
	}
	return defaultValue
}

func getEnvAsDuration(name string, defaultValue time.Duration) time.Duration {
	if valueStr, exists := os.LookupEnv(name); exists {
		if value, err := time.ParseDuration(valueStr); err == nil && value >= 0 {
			return value
		}
	}
	return defaultValue
}

func NewTLSConfig() (*tls.Config, error) {
	private, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, err
	}

	serialNumber, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return nil, err
	}

	template := x509.Certificate{
		SerialNumber: serialNumber,
		NotBefore:    time.Now(),
		NotAfter:     time.Now().AddDate(1, 0, 0),
		KeyUsage:     x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}

	crtBytes, err := x509.CreateCertificate(rand.Reader, &template, &template, &private.PublicKey, private)
	if err != nil {
		return nil, err
	}

	keyBytes, err := x509.MarshalPKCS8PrivateKey(private)
	if err != nil {
		return nil, err
	}

	crtPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: crtBytes})
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: keyBytes})

	cert, err := tls.X509KeyPair(crtPEM, keyPEM)
	if err != nil {
		return nil, err
	}

	return &tls.Config{Certificates: []tls.Certificate{cert}}, nil
}

func (c *Common) formatCertFingerprint(certRaw []byte) string {
	hash := sha256.Sum256(certRaw)
	hashHex := hex.EncodeToString(hash[:])

	var formatted strings.Builder
	for i := 0; i < len(hashHex); i += 2 {
		if i > 0 {
			formatted.WriteByte(':')
		}
		formatted.WriteString(strings.ToUpper(hashHex[i : i+2]))
	}

	return "sha256:" + formatted.String()
}

func (c *Common) xor(data []byte) []byte {
	for i := range data {
		data[i] ^= c.tunnelKey[i%len(c.tunnelKey)]
	}
	return data
}

func (c *Common) generateAuthToken() string {
	return hex.EncodeToString(hmac.New(sha256.New, []byte(c.tunnelKey)).Sum(nil))
}

func (c *Common) verifyAuthToken(token string) bool {
	return hmac.Equal([]byte(token), []byte(c.generateAuthToken()))
}

func (c *Common) encode(data []byte) []byte {
	return append([]byte(base64.StdEncoding.EncodeToString(c.xor(data))), '\n')
}

func (c *Common) decode(data []byte) ([]byte, error) {
	decoded, err := base64.StdEncoding.DecodeString(string(bytes.TrimSuffix(data, []byte{'\n'})))
	if err != nil {
		return nil, fmt.Errorf("decode: base64 decode failed: %w", err)
	}
	return c.xor(decoded), nil
}

func (c *Common) resolve(network, address string) (any, error) {
	now := time.Now()

	if val, ok := c.dnsCacheEntries.Load(address); ok {
		entry := val.(*dnsCacheEntry)
		if now.Before(entry.expiredAt) {
			if network == "tcp" {
				return entry.tcpAddr, nil
			}
			return entry.udpAddr, nil
		}
		c.dnsCacheEntries.Delete(address)
	}

	tcpAddr, err := net.ResolveTCPAddr("tcp", address)
	if err != nil {
		return nil, fmt.Errorf("resolve: resolveTCPAddr failed: %w", err)
	}

	udpAddr, err := net.ResolveUDPAddr("udp", address)
	if err != nil {
		return nil, fmt.Errorf("resolve: resolveUDPAddr failed: %w", err)
	}

	entry := &dnsCacheEntry{
		tcpAddr:   tcpAddr,
		udpAddr:   udpAddr,
		expiredAt: now.Add(c.dnsCacheTTL),
	}
	c.dnsCacheEntries.LoadOrStore(address, entry)

	if network == "tcp" {
		return tcpAddr, nil
	}
	return udpAddr, nil
}

func (c *Common) clearCache() {
	c.dnsCacheEntries.Range(func(key, value any) bool {
		c.dnsCacheEntries.Delete(key)
		return true
	})
}

func (c *Common) resolveAddr(network, address string) (any, error) {
	host, _, err := net.SplitHostPort(address)
	if err != nil {
		return nil, fmt.Errorf("invalid address %s: %w", address, err)
	}

	if host == "" || net.ParseIP(host) != nil {
		if network == "tcp" {
			return net.ResolveTCPAddr("tcp", address)
		}
		return net.ResolveUDPAddr("udp", address)
	}

	return c.resolve(network, address)
}

func (c *Common) resolveTarget(network string, idx int) (any, error) {
	if idx < 0 || idx >= len(c.targetAddrs) {
		return nil, fmt.Errorf("resolveTarget: index %d out of range", idx)
	}

	addr, err := c.resolveAddr(network, c.targetAddrs[idx])
	if err != nil {
		if network == "tcp" {
			return c.targetTCPAddrs[idx], err
		}
		return c.targetUDPAddrs[idx], err
	}
	return addr, nil
}

func (c *Common) getTunnelTCPAddr() (*net.TCPAddr, error) {
	addr, err := c.resolveAddr("tcp", c.tunnelAddr)
	if err != nil {
		return c.tunnelTCPAddr, err
	}
	return addr.(*net.TCPAddr), nil
}

func (c *Common) getTunnelUDPAddr() (*net.UDPAddr, error) {
	addr, err := c.resolveAddr("udp", c.tunnelAddr)
	if err != nil {
		return c.tunnelUDPAddr, err
	}
	return addr.(*net.UDPAddr), nil
}

func (c *Common) getTargetAddrsString() string {
	addrs := make([]string, len(c.targetTCPAddrs))
	for i, addr := range c.targetTCPAddrs {
		addrs[i] = addr.String()
	}
	return strings.Join(addrs, ",")
}

func (c *Common) nextTargetIdx() int {
	if len(c.targetTCPAddrs) <= 1 {
		return 0
	}
	return int((atomic.AddUint64(&c.targetIdx, 1) - 1) % uint64(len(c.targetTCPAddrs)))
}

func (c *Common) probeBestTarget() int {
	count := len(c.targetTCPAddrs)
	if count == 0 {
		return 0
	}

	type result struct{ idx, lat int }
	results := make(chan result, count)
	for i := range count {
		go func(idx int) { results <- result{idx, c.tcpPing(idx)} }(i)
	}

	bestIdx, bestLat := 0, 0
	for range count {
		if r := <-results; r.lat > 0 && (bestLat == 0 || r.lat < bestLat) {
			bestIdx, bestLat = r.idx, r.lat
		}
	}

	if bestLat > 0 {
		atomic.StoreUint64(&c.targetIdx, uint64(bestIdx))
		atomic.StoreInt32(&c.bestLatency, int32(bestLat))
	}
	return bestLat
}

func (c *Common) tcpPing(idx int) int {
	addr, _ := c.resolveTarget("tcp", idx)
	if tcpAddr, ok := addr.(*net.TCPAddr); ok {
		start := time.Now()
		if conn, err := net.DialTimeout("tcp", tcpAddr.String(), reportInterval); err == nil {
			conn.Close()
			return int(time.Since(start).Milliseconds())
		}
	}
	return 0
}

func (c *Common) dialWithRotation(network string, timeout time.Duration) (net.Conn, error) {
	addrCount := len(c.targetAddrs)

	getAddr := func(i int) string {
		addr, _ := c.resolveTarget(network, i)
		if tcpAddr, ok := addr.(*net.TCPAddr); ok {
			return tcpAddr.String()
		}
		if udpAddr, ok := addr.(*net.UDPAddr); ok {
			return udpAddr.String()
		}
		return ""
	}

	dialer := &net.Dialer{Timeout: timeout}
	if c.dialerIP != defaultDialerIP && atomic.LoadUint32(&c.dialerFallback) == 0 {
		if network == "tcp" {
			dialer.LocalAddr = &net.TCPAddr{IP: net.ParseIP(c.dialerIP)}
		} else {
			dialer.LocalAddr = &net.UDPAddr{IP: net.ParseIP(c.dialerIP)}
		}
	}

	tryDial := func(addr string) (net.Conn, error) {
		conn, err := dialer.Dial(network, addr)
		if err != nil && dialer.LocalAddr != nil && atomic.CompareAndSwapUint32(&c.dialerFallback, 0, 1) {
			c.logger.Error("dialWithRotation: fallback to system auto due to dialer failure: %v", err)
			dialer.LocalAddr = nil
			return dialer.Dial(network, addr)
		}
		return conn, err
	}

	if addrCount == 1 {
		if addr := getAddr(0); addr != "" {
			return tryDial(addr)
		}
		return nil, fmt.Errorf("dialWithRotation: invalid target address")
	}

	var startIdx int
	switch c.lbStrategy {
	case "1":
		startIdx = int(atomic.LoadUint64(&c.targetIdx) % uint64(addrCount))
	case "2":
		now := uint64(time.Now().UnixNano())
		last := atomic.LoadUint64(&c.lastFallback)
		if now-last > uint64(fallbackInterval) {
			atomic.StoreUint64(&c.lastFallback, now)
			atomic.StoreUint64(&c.targetIdx, 0)
		}
		startIdx = int(atomic.LoadUint64(&c.targetIdx) % uint64(addrCount))
	default:
		startIdx = c.nextTargetIdx()
	}

	var lastErr error
	for i := range addrCount {
		targetIdx := (startIdx + i) % addrCount
		addr := getAddr(targetIdx)
		if addr == "" {
			continue
		}
		conn, err := tryDial(addr)
		if err == nil {
			if i > 0 && (c.lbStrategy == "1" || c.lbStrategy == "2") {
				atomic.StoreUint64(&c.targetIdx, uint64(targetIdx))
			}
			return conn, nil
		}
		lastErr = err
	}

	return nil, fmt.Errorf("dialWithRotation: all %d targets failed: %w", addrCount, lastErr)
}

func (c *Common) getAddress() error {
	tunnelAddr := c.parsedURL.Host
	if tunnelAddr == "" {
		return fmt.Errorf("getAddress: no valid tunnel address found")
	}

	c.tunnelAddr = tunnelAddr
	if name, port, err := net.SplitHostPort(tunnelAddr); err == nil {
		c.serverName, c.serverPort = name, port
	}

	tcpAddr, err := c.resolveAddr("tcp", tunnelAddr)
	if err != nil {
		return fmt.Errorf("getAddress: resolveTCPAddr failed: %w", err)
	}
	c.tunnelTCPAddr = tcpAddr.(*net.TCPAddr)

	udpAddr, err := c.resolveAddr("udp", tunnelAddr)
	if err != nil {
		return fmt.Errorf("getAddress: resolveUDPAddr failed: %w", err)
	}
	c.tunnelUDPAddr = udpAddr.(*net.UDPAddr)

	targetAddr := strings.TrimPrefix(c.parsedURL.Path, "/")
	if targetAddr == "" {
		return fmt.Errorf("getAddress: no valid target address found")
	}

	addrList := strings.Split(targetAddr, ",")
	tempTCPAddrs := make([]*net.TCPAddr, 0, len(addrList))
	tempUDPAddrs := make([]*net.UDPAddr, 0, len(addrList))
	tempRawAddrs := make([]string, 0, len(addrList))

	for _, addr := range addrList {
		addr = strings.TrimSpace(addr)
		if addr == "" {
			continue
		}

		tcpAddr, err := c.resolveAddr("tcp", addr)
		if err != nil {
			return fmt.Errorf("getAddress: resolveTCPAddr failed for %s: %w", addr, err)
		}

		udpAddr, err := c.resolveAddr("udp", addr)
		if err != nil {
			return fmt.Errorf("getAddress: resolveUDPAddr failed for %s: %w", addr, err)
		}

		tempTCPAddrs = append(tempTCPAddrs, tcpAddr.(*net.TCPAddr))
		tempUDPAddrs = append(tempUDPAddrs, udpAddr.(*net.UDPAddr))
		tempRawAddrs = append(tempRawAddrs, addr)
	}

	if len(tempTCPAddrs) == 0 || len(tempUDPAddrs) == 0 || len(tempTCPAddrs) != len(tempUDPAddrs) {
		return fmt.Errorf("getAddress: no valid target address found")
	}

	c.targetAddrs = tempRawAddrs
	c.targetTCPAddrs = tempTCPAddrs
	c.targetUDPAddrs = tempUDPAddrs
	c.targetIdx = 0

	tunnelPort := c.tunnelTCPAddr.Port
	for _, targetAddr := range c.targetTCPAddrs {
		if targetAddr.Port == tunnelPort && (targetAddr.IP.IsLoopback() || c.tunnelTCPAddr.IP.IsUnspecified()) {
			return fmt.Errorf("getAddress: tunnel port %d conflicts with target address %s", tunnelPort, targetAddr.String())
		}
	}

	return nil
}

func (c *Common) getCoreType() {
	c.coreType = c.parsedURL.Scheme
}

func (c *Common) getTunnelKey() {
	if key := c.parsedURL.User.Username(); key != "" {
		c.tunnelKey = key
	} else {
		hash := fnv.New32a()
		hash.Write([]byte(c.parsedURL.Port()))
		c.tunnelKey = hex.EncodeToString(hash.Sum(nil))
	}
}

func (c *Common) getDNSTTL() {
	if dns := c.parsedURL.Query().Get("dns"); dns != "" {
		if ttl, err := time.ParseDuration(dns); err == nil && ttl > 0 {
			c.dnsCacheTTL = ttl
		}
	} else {
		c.dnsCacheTTL = defaultDNSTTL
	}
}

func (c *Common) getServerName() {
	if serverName := c.parsedURL.Query().Get("sni"); serverName != "" {
		c.serverName = serverName
		return
	}
	if c.serverName == "" || net.ParseIP(c.serverName) != nil {
		c.serverName = defaultServerName
	}
}

func (c *Common) getLBStrategy() {
	if lbStrategy := c.parsedURL.Query().Get("lbs"); lbStrategy != "" {
		c.lbStrategy = lbStrategy
	} else {
		c.lbStrategy = defaultLBStrategy
	}
}

func (c *Common) getPoolCapacity() {
	if min := c.parsedURL.Query().Get("min"); min != "" {
		if value, err := strconv.Atoi(min); err == nil && value > 0 {
			c.minPoolCapacity = value
		}
	} else {
		c.minPoolCapacity = defaultMinPool
	}

	if max := c.parsedURL.Query().Get("max"); max != "" {
		if value, err := strconv.Atoi(max); err == nil && value > 0 {
			c.maxPoolCapacity = value
		}
	} else {
		c.maxPoolCapacity = defaultMaxPool
	}
}

func (c *Common) getRunMode() {
	if mode := c.parsedURL.Query().Get("mode"); mode != "" {
		c.runMode = mode
	} else {
		c.runMode = defaultRunMode
	}
}

func (c *Common) getPoolType() {
	if poolType := c.parsedURL.Query().Get("type"); poolType != "" {
		c.poolType = poolType
	} else {
		c.poolType = defaultPoolType
	}
	if c.poolType == "1" && c.tlsCode == "0" {
		c.tlsCode = "1"
	}
}

func (c *Common) getDialerIP() {
	if dialerIP := c.parsedURL.Query().Get("dial"); dialerIP != "" && dialerIP != "auto" {
		if ip := net.ParseIP(dialerIP); ip != nil {
			c.dialerIP = dialerIP
			return
		} else {
			c.logger.Error("getDialerIP: fallback to system auto due to invalid IP address: %v", dialerIP)
		}
	}
	c.dialerIP = defaultDialerIP
}

func (c *Common) getReadTimeout() {
	if timeout := c.parsedURL.Query().Get("read"); timeout != "" {
		if value, err := time.ParseDuration(timeout); err == nil && value > 0 {
			c.readTimeout = value
		}
	} else {
		c.readTimeout = defaultReadTimeout
	}
}

func (c *Common) getRateLimit() {
	if limit := c.parsedURL.Query().Get("rate"); limit != "" {
		if value, err := strconv.Atoi(limit); err == nil && value > 0 {
			c.rateLimit = value * 125000
		}
	} else {
		c.rateLimit = defaultRateLimit
	}
}

func (c *Common) getSlotLimit() {
	if slot := c.parsedURL.Query().Get("slot"); slot != "" {
		if value, err := strconv.Atoi(slot); err == nil && value > 0 {
			c.slotLimit = int32(value)
		}
	} else {
		c.slotLimit = defaultSlotLimit
	}
}

func (c *Common) getProxyProtocol() {
	if protocol := c.parsedURL.Query().Get("proxy"); protocol != "" {
		c.proxyProtocol = protocol
	} else {
		c.proxyProtocol = defaultProxyProtocol
	}
}

func (c *Common) getBlockProtocol() {
	if protocol := c.parsedURL.Query().Get("block"); protocol != "" {
		c.blockProtocol = protocol
	} else {
		c.blockProtocol = defaultBlockProtocol
	}
	c.blockSOCKS = strings.Contains(c.blockProtocol, "1")
	c.blockHTTP = strings.Contains(c.blockProtocol, "2")
	c.blockTLS = strings.Contains(c.blockProtocol, "3")
}

func (c *Common) getTCPStrategy() {
	if tcpStrategy := c.parsedURL.Query().Get("notcp"); tcpStrategy != "" {
		c.disableTCP = tcpStrategy
	} else {
		c.disableTCP = defaultTCPStrategy
	}
}

func (c *Common) getUDPStrategy() {
	if udpStrategy := c.parsedURL.Query().Get("noudp"); udpStrategy != "" {
		c.disableUDP = udpStrategy
	} else {
		c.disableUDP = defaultUDPStrategy
	}
}

func (c *Common) initConfig() error {
	if err := c.getAddress(); err != nil {
		return err
	}

	c.getCoreType()
	c.getDNSTTL()
	c.getTunnelKey()
	c.getPoolCapacity()
	c.getServerName()
	c.getLBStrategy()
	c.getRunMode()
	c.getPoolType()
	c.getDialerIP()
	c.getReadTimeout()
	c.getRateLimit()
	c.getSlotLimit()
	c.getProxyProtocol()
	c.getBlockProtocol()
	c.getTCPStrategy()
	c.getUDPStrategy()

	return nil
}

func (c *Common) sendProxyV1Header(ip string, conn net.Conn) error {
	if c.proxyProtocol != "1" {
		return nil
	}

	clientAddr, err := net.ResolveTCPAddr("tcp", ip)
	if err != nil {
		return fmt.Errorf("sendProxyV1Header: resolveTCPAddr failed: %w", err)
	}
	remoteAddr, ok := conn.RemoteAddr().(*net.TCPAddr)
	if !ok {
		return fmt.Errorf("sendProxyV1Header: remote address is not TCPAddr")
	}

	var protocol string
	switch {
	case clientAddr.IP.To4() != nil && remoteAddr.IP.To4() != nil:
		protocol = "TCP4"
	case clientAddr.IP.To16() != nil && remoteAddr.IP.To16() != nil:
		protocol = "TCP6"
	default:
		return fmt.Errorf("sendProxyV1Header: unsupported IP protocol for PROXY v1")
	}

	if _, err = fmt.Fprintf(conn, "PROXY %s %s %s %d %d\r\n",
		protocol,
		clientAddr.IP.String(),
		remoteAddr.IP.String(),
		clientAddr.Port,
		remoteAddr.Port); err != nil {
		return fmt.Errorf("sendProxyV1Header: fprintf failed: %w", err)
	}

	return nil
}

func (c *Common) detectBlockProtocol(conn net.Conn) (string, net.Conn) {
	if !c.blockSOCKS && !c.blockHTTP && !c.blockTLS {
		return "", conn
	}

	reader := bufio.NewReader(conn)
	b, err := reader.Peek(8)
	if err != nil || len(b) < 1 {
		return "", &readerConn{Conn: conn, reader: reader}
	}

	if c.blockSOCKS && len(b) >= 2 {
		if b[0] == 0x04 && (b[1] == 0x01 || b[1] == 0x02) {
			return "SOCKS4", &readerConn{Conn: conn, reader: reader}
		}
		if b[0] == 0x05 && b[1] >= 0x01 && b[1] <= 0x03 {
			return "SOCKS5", &readerConn{Conn: conn, reader: reader}
		}
	}

	if c.blockHTTP && len(b) >= 4 && b[0] >= 'A' && b[0] <= 'Z' {
		for i, c := range b[1:] {
			if c == ' ' {
				return "HTTP", &readerConn{Conn: conn, reader: reader}
			}
			if c < 'A' || c > 'Z' || i >= 7 {
				break
			}
		}
	}

	if c.blockTLS && b[0] == 0x16 {
		return "TLS", &readerConn{Conn: conn, reader: reader}
	}

	return "", &readerConn{Conn: conn, reader: reader}
}

func (c *Common) initRateLimiter() {
	if c.rateLimit > 0 {
		c.rateLimiter = conn.NewRateLimiter(int64(c.rateLimit), int64(c.rateLimit))
	}
}

func (c *Common) initContext() {
	if c.cancel != nil {
		c.cancel()
	}
	c.ctx, c.cancel = context.WithCancel(context.Background())
}

func (c *Common) initTunnelListener() error {
	if c.tunnelTCPAddr == nil && c.tunnelUDPAddr == nil {
		return fmt.Errorf("initTunnelListener: nil tunnel address")
	}

	if c.tunnelTCPAddr != nil && (c.disableTCP != "1" || c.coreType != "client") {
		tunnelListener, err := net.ListenTCP("tcp", c.tunnelTCPAddr)
		if err != nil {
			return fmt.Errorf("initTunnelListener: listenTCP failed: %w", err)
		}
		c.tunnelListener = tunnelListener
	}

	if c.tunnelUDPAddr != nil && (c.disableUDP != "1" || c.coreType != "client") {
		tunnelUDPConn, err := net.ListenUDP("udp", c.tunnelUDPAddr)
		if err != nil {
			return fmt.Errorf("initTunnelListener: listenUDP failed: %w", err)
		}
		c.tunnelUDPConn = &conn.StatConn{Conn: tunnelUDPConn, RX: &c.udpRX, TX: &c.udpTX, Rate: c.rateLimiter}
	}

	return nil
}

func (c *Common) initTargetListener() error {
	if len(c.targetAddrs) == 0 {
		return fmt.Errorf("initTargetListener: no target address")
	}

	if len(c.targetTCPAddrs) > 0 && c.disableTCP != "1" {
		targetListener, err := net.ListenTCP("tcp", c.targetTCPAddrs[0])
		if err != nil {
			return fmt.Errorf("initTargetListener: listenTCP failed: %w", err)
		}
		c.targetListener = targetListener
	}

	if len(c.targetUDPAddrs) > 0 && c.disableUDP != "1" {
		targetUDPConn, err := net.ListenUDP("udp", c.targetUDPAddrs[0])
		if err != nil {
			return fmt.Errorf("initTargetListener: listenUDP failed: %w", err)
		}
		c.targetUDPConn = &conn.StatConn{Conn: targetUDPConn, RX: &c.udpRX, TX: &c.udpTX, Rate: c.rateLimiter}
	}

	return nil
}

func drain[T any](ch <-chan T) {
	for {
		select {
		case <-ch:
		default:
			return
		}
	}
}

func (c *Common) stop() {
	if c.cancel != nil {
		c.cancel()
	}

	if c.tunnelPool != nil {
		active := c.tunnelPool.Active()
		c.tunnelPool.Close()
		c.logger.Debug("Tunnel connection closed: pool active %v", active)
	}

	c.targetUDPSession.Range(func(key, value any) bool {
		if conn, ok := value.(*net.UDPConn); ok {
			conn.Close()
		}
		c.targetUDPSession.Delete(key)
		return true
	})

	if c.targetUDPConn != nil {
		c.targetUDPConn.Close()
		c.logger.Debug("Target connection closed: %v", c.targetUDPConn.LocalAddr())
	}

	if c.tunnelUDPConn != nil {
		c.tunnelUDPConn.Close()
		c.logger.Debug("Tunnel connection closed: %v", c.tunnelUDPConn.LocalAddr())
	}

	if c.controlConn != nil {
		c.controlConn.Close()
		c.logger.Debug("Control connection closed: %v", c.controlConn.LocalAddr())
	}

	if c.targetListener != nil {
		c.targetListener.Close()
		c.logger.Debug("Target listener closed: %v", c.targetListener.Addr())
	}

	if c.tunnelListener != nil {
		c.tunnelListener.Close()
		c.logger.Debug("Tunnel listener closed: %v", c.tunnelListener.Addr())
	}

	drain(c.signalChan)
	drain(c.writeChan)
	drain(c.verifyChan)

	if c.rateLimiter != nil {
		c.rateLimiter.Reset()
	}

	c.clearCache()
}

func (c *Common) shutdown(ctx context.Context, stopFunc func()) error {
	done := make(chan struct{})
	go func() {
		defer close(done)
		stopFunc()
	}()

	select {
	case <-ctx.Done():
		return fmt.Errorf("shutdown: context error: %w", ctx.Err())
	case <-done:
		return nil
	}
}

func (c *Common) setControlConn() error {
	start := time.Now()
	for c.ctx.Err() == nil {
		if c.tunnelPool.Ready() && c.tunnelPool.Active() > 0 {
			break
		}
		if time.Since(start) > handshakeTimeout {
			return fmt.Errorf("setControlConn: handshake timeout")
		}
		select {
		case <-c.ctx.Done():
			return fmt.Errorf("setControlConn: context error: %w", c.ctx.Err())
		case <-time.After(contextCheckInterval):
		}
	}

	poolConn, err := c.tunnelPool.OutgoingGet("00000000", poolGetTimeout)
	if err != nil {
		return fmt.Errorf("setControlConn: outgoingGet failed: %w", err)
	}
	c.controlConn = poolConn
	c.bufReader = bufio.NewReader(&conn.TimeoutReader{Conn: c.controlConn, Timeout: 3 * reportInterval})
	c.logger.Info("Marking tunnel handshake as complete in %vms", time.Since(c.handshakeStart).Milliseconds())

	go func() {
		for {
			select {
			case <-c.ctx.Done():
				return
			case data := <-c.writeChan:
				_, err := c.controlConn.Write(data)
				if err != nil {
					c.logger.Error("startWriter: write failed: %v", err)
				}
			}
		}
	}()

	if c.tlsCode == "1" {
		c.logger.Info("TLS code-1: RAM cert fingerprint verifying...")
	}
	return nil
}

func (c *Common) commonControl() error {
	errChan := make(chan error, 3)

	go func() { errChan <- c.commonOnce() }()
	go func() { errChan <- c.commonQueue() }()
	go func() { errChan <- c.healthCheck() }()

	select {
	case <-c.ctx.Done():
		return fmt.Errorf("commonControl: context error: %w", c.ctx.Err())
	case err := <-errChan:
		return fmt.Errorf("commonControl: %w", err)
	}
}

func (c *Common) commonQueue() error {
	for c.ctx.Err() == nil {
		rawSignal, err := c.bufReader.ReadBytes('\n')
		if err != nil {
			return fmt.Errorf("commonQueue: readBytes failed: %w", err)
		}

		signalData, err := c.decode(rawSignal)
		if err != nil {
			c.logger.Error("commonQueue: decode signal failed: %v", err)
			select {
			case <-c.ctx.Done():
				return fmt.Errorf("commonQueue: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		var signal Signal
		if err := json.Unmarshal(signalData, &signal); err != nil {
			c.logger.Error("commonQueue: unmarshal signal failed: %v", err)
			select {
			case <-c.ctx.Done():
				return fmt.Errorf("commonQueue: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		select {
		case c.signalChan <- signal:
		default:
			c.logger.Error("commonQueue: queue limit reached: %v", semaphoreLimit)
			select {
			case <-c.ctx.Done():
				return fmt.Errorf("commonQueue: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
		}
	}

	return fmt.Errorf("commonQueue: context error: %w", c.ctx.Err())
}

func (c *Common) healthCheck() error {
	ticker := time.NewTicker(reportInterval)
	defer ticker.Stop()

	if c.tlsCode == "1" {
		go func() {
			select {
			case <-c.ctx.Done():
			case <-time.After(reportInterval):
				c.incomingVerify()
			}
		}()
	}

	for c.ctx.Err() == nil {
		if c.tunnelPool.ErrorCount() > c.tunnelPool.Active()/2 {
			if c.ctx.Err() == nil && c.controlConn != nil {
				signalData, _ := json.Marshal(Signal{ActionType: "flush"})
				c.writeChan <- c.encode(signalData)
			}
			c.tunnelPool.Flush()
			c.tunnelPool.ResetError()

			select {
			case <-c.ctx.Done():
				return fmt.Errorf("healthCheck: context error: %w", c.ctx.Err())
			case <-ticker.C:
			}

			c.logger.Debug("Tunnel pool flushed: %v active connections", c.tunnelPool.Active())
		}

		if c.lbStrategy == "1" && len(c.targetTCPAddrs) > 1 {
			c.probeBestTarget()
		}

		c.checkPoint = time.Now()
		if c.ctx.Err() == nil && c.controlConn != nil {
			signalData, _ := json.Marshal(Signal{ActionType: "ping"})
			c.writeChan <- c.encode(signalData)
		}
		select {
		case <-c.ctx.Done():
			return fmt.Errorf("healthCheck: context error: %w", c.ctx.Err())
		case <-ticker.C:
		}
	}

	return fmt.Errorf("healthCheck: context error: %w", c.ctx.Err())
}

func (c *Common) incomingVerify() {
	for c.ctx.Err() == nil {
		if c.tunnelPool.Ready() && c.tunnelPool.Active() > 0 {
			break
		}
		select {
		case <-c.ctx.Done():
			continue
		case <-time.After(contextCheckInterval):
		}
	}

	id, testConn, err := c.tunnelPool.IncomingGet(poolGetTimeout)
	if err != nil {
		c.logger.Error("incomingVerify: incomingGet failed: %v", err)
		c.cancel()
		return
	}
	defer testConn.Close()

	var fingerprint string
	switch c.coreType {
	case "server":
		if c.tlsConfig != nil && len(c.tlsConfig.Certificates) > 0 {
			cert := c.tlsConfig.Certificates[0]
			if len(cert.Certificate) > 0 {
				fingerprint = c.formatCertFingerprint(cert.Certificate[0])
			}
		}
	case "client":
		if conn, ok := testConn.(interface{ ConnectionState() tls.ConnectionState }); ok {
			state := conn.ConnectionState()
			if len(state.PeerCertificates) > 0 {
				fingerprint = c.formatCertFingerprint(state.PeerCertificates[0].Raw)
			}
		}
	}

	if c.ctx.Err() == nil && c.controlConn != nil {
		signalData, _ := json.Marshal(Signal{
			ActionType:  "verify",
			PoolConnID:  id,
			Fingerprint: fingerprint,
		})
		c.writeChan <- c.encode(signalData)
	}

	c.logger.Debug("TLS code-1: verify signal: cid %v -> %v", id, c.controlConn.RemoteAddr())
}

func (c *Common) commonLoop() {
	for c.ctx.Err() == nil {
		if c.tunnelPool.Ready() {
			if c.tlsCode == "1" {
				select {
				case <-c.verifyChan:
				case <-c.ctx.Done():
					return
				}
			}

			if c.targetListener != nil || c.disableTCP != "1" {
				go c.commonTCPLoop()
			}
			if c.targetUDPConn != nil || c.disableUDP != "1" {
				go c.commonUDPLoop()
			}
			return
		}

		select {
		case <-c.ctx.Done():
			return
		case <-time.After(contextCheckInterval):
		}
	}
}

func (c *Common) commonTCPLoop() {
	for c.ctx.Err() == nil {
		targetConn, err := c.targetListener.Accept()
		if err != nil {
			if c.ctx.Err() != nil || err == net.ErrClosed {
				return
			}
			c.logger.Error("commonTCPLoop: accept failed: %v", err)

			select {
			case <-c.ctx.Done():
				return
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		targetConn = &conn.StatConn{Conn: targetConn, RX: &c.tcpRX, TX: &c.tcpTX, Rate: c.rateLimiter}
		c.logger.Debug("Target connection: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())

		go func(targetConn net.Conn) {
			defer func() {
				if targetConn != nil {
					targetConn.Close()
				}
			}()

			if !c.tryAcquireSlot(false) {
				c.logger.Error("commonTCPLoop: TCP slot limit reached: %v/%v", c.tcpSlot, c.slotLimit)
				return
			}
			defer c.releaseSlot(false)

			protocol, wrappedConn := c.detectBlockProtocol(targetConn)
			if protocol != "" {
				c.logger.Warn("commonTCPLoop: blocked %v protocol from %v", protocol, targetConn.RemoteAddr())
				return
			}
			targetConn = wrappedConn

			id, remoteConn, err := c.tunnelPool.IncomingGet(poolGetTimeout)
			if err != nil {
				c.logger.Warn("commonTCPLoop: request timeout: %v", err)
				return
			}

			c.logger.Debug("Tunnel connection: get %v <- pool active %v", id, c.tunnelPool.Active())

			defer func() {
				if remoteConn != nil {
					remoteConn.Close()
					c.logger.Debug("Tunnel connection: closed %v", id)
				}
			}()

			c.logger.Debug("Tunnel connection: %v <-> %v", remoteConn.LocalAddr(), remoteConn.RemoteAddr())

			if c.ctx.Err() == nil && c.controlConn != nil {
				signalData, _ := json.Marshal(Signal{
					ActionType: "tcp",
					RemoteAddr: targetConn.RemoteAddr().String(),
					PoolConnID: id,
				})
				c.writeChan <- c.encode(signalData)
			}

			c.logger.Debug("TCP launch signal: cid %v -> %v", id, c.controlConn.RemoteAddr())

			buffer1 := c.getTCPBuffer()
			buffer2 := c.getTCPBuffer()
			defer func() {
				c.putTCPBuffer(buffer1)
				c.putTCPBuffer(buffer2)
			}()

			c.logger.Info("Starting exchange: %v <-> %v", targetConn.RemoteAddr(), remoteConn.RemoteAddr())
			c.logger.Info("Exchange complete: %v", conn.DataExchange(targetConn, remoteConn, c.readTimeout, buffer1, buffer2))
		}(targetConn)
	}
}

func (c *Common) commonUDPLoop() {
	for c.ctx.Err() == nil {
		buffer := c.getUDPBuffer()

		x, clientAddr, err := c.targetUDPConn.ReadFromUDP(buffer)
		if err != nil {
			if c.ctx.Err() != nil || err == net.ErrClosed {
				c.putUDPBuffer(buffer)
				return
			}
			c.logger.Error("commonUDPLoop: readFromUDP failed: %v", err)
			c.putUDPBuffer(buffer)

			select {
			case <-c.ctx.Done():
				return
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		c.logger.Debug("Target connection: %v <-> %v", c.targetUDPConn.LocalAddr(), clientAddr)

		var id string
		var remoteConn net.Conn
		sessionKey := clientAddr.String()

		if session, ok := c.targetUDPSession.Load(sessionKey); ok {
			remoteConn = session.(net.Conn)
			c.logger.Debug("Using UDP session: %v <-> %v", remoteConn.LocalAddr(), remoteConn.RemoteAddr())
		} else {
			if !c.tryAcquireSlot(true) {
				c.logger.Error("commonUDPLoop: UDP slot limit reached: %v/%v", c.udpSlot, c.slotLimit)
				c.putUDPBuffer(buffer)
				continue
			}

			id, remoteConn, err = c.tunnelPool.IncomingGet(poolGetTimeout)
			if err != nil {
				c.logger.Warn("commonUDPLoop: request timeout: %v", err)
				c.releaseSlot(true)
				c.putUDPBuffer(buffer)
				continue
			}
			c.targetUDPSession.Store(sessionKey, remoteConn)
			c.logger.Debug("Tunnel connection: get %v <- pool active %v", id, c.tunnelPool.Active())
			c.logger.Debug("Tunnel connection: %v <-> %v", remoteConn.LocalAddr(), remoteConn.RemoteAddr())

			go func(remoteConn net.Conn, clientAddr *net.UDPAddr, sessionKey, id string) {
				defer func() {
					c.targetUDPSession.Delete(sessionKey)
					c.releaseSlot(true)

					if remoteConn != nil {
						remoteConn.Close()
						c.logger.Debug("Tunnel connection: closed %v", id)
					}
				}()

				buffer := c.getUDPBuffer()
				defer c.putUDPBuffer(buffer)
				reader := &conn.TimeoutReader{Conn: remoteConn, Timeout: udpReadTimeout}

				for c.ctx.Err() == nil {
					x, err := reader.Read(buffer)
					if err != nil {
						if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
							c.logger.Debug("UDP session abort: %v", err)
						} else if err != io.EOF {
							c.logger.Error("commonUDPLoop: read from tunnel failed: %v", err)
						}
						return
					}

					_, err = c.targetUDPConn.WriteToUDP(buffer[:x], clientAddr)
					if err != nil {
						if err != io.EOF {
							c.logger.Error("commonUDPLoop: writeToUDP failed: %v", err)
						}
						return
					}
					c.logger.Debug("Transfer complete: %v <-> %v", remoteConn.LocalAddr(), c.targetUDPConn.LocalAddr())
				}
			}(remoteConn, clientAddr, sessionKey, id)

			if c.ctx.Err() == nil && c.controlConn != nil {
				signalData, _ := json.Marshal(Signal{
					ActionType: "udp",
					RemoteAddr: clientAddr.String(),
					PoolConnID: id,
				})
				c.writeChan <- c.encode(signalData)
			}

			c.logger.Debug("UDP launch signal: cid %v -> %v", id, c.controlConn.RemoteAddr())
			c.logger.Debug("Starting transfer: %v <-> %v", remoteConn.LocalAddr(), c.targetUDPConn.LocalAddr())
		}

		_, err = remoteConn.Write(buffer[:x])
		if err != nil {
			if err != io.EOF {
				c.logger.Error("commonUDPLoop: write to tunnel failed: %v", err)
			}
			c.targetUDPSession.Delete(sessionKey)
			remoteConn.Close()
			c.putUDPBuffer(buffer)
			continue
		}

		c.logger.Debug("Transfer complete: %v <-> %v", remoteConn.LocalAddr(), c.targetUDPConn.LocalAddr())
		c.putUDPBuffer(buffer)
	}
}

func (c *Common) commonOnce() error {
	for c.ctx.Err() == nil {
		if !c.tunnelPool.Ready() {
			select {
			case <-c.ctx.Done():
				return fmt.Errorf("commonOnce: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		select {
		case <-c.ctx.Done():
			return fmt.Errorf("commonOnce: context error: %w", c.ctx.Err())
		case signal := <-c.signalChan:
			switch signal.ActionType {
			case "verify":
				if c.tlsCode == "1" {
					go c.outgoingVerify(signal)
				}
			case "tcp":
				if c.disableTCP != "1" {
					go c.commonTCPOnce(signal)
				}
			case "udp":
				if c.disableUDP != "1" {
					go c.commonUDPOnce(signal)
				}
			case "flush":
				go func() {
					c.tunnelPool.Flush()
					c.tunnelPool.ResetError()

					select {
					case <-c.ctx.Done():
						return
					case <-time.After(reportInterval):
					}

					c.logger.Debug("Tunnel pool flushed: %v active connections", c.tunnelPool.Active())
				}()
			case "ping":
				if c.ctx.Err() == nil && c.controlConn != nil {
					signalData, _ := json.Marshal(Signal{ActionType: "pong"})
					c.writeChan <- c.encode(signalData)
				}
			case "pong":
				c.logger.Event("CHECK_POINT|MODE=%v|PING=%vms|POOL=%v|TCPS=%v|UDPS=%v|TCPRX=%v|TCPTX=%v|UDPRX=%v|UDPTX=%v",
					c.runMode, time.Since(c.checkPoint).Milliseconds(), c.tunnelPool.Active(),
					atomic.LoadInt32(&c.tcpSlot), atomic.LoadInt32(&c.udpSlot),
					atomic.LoadUint64(&c.tcpRX), atomic.LoadUint64(&c.tcpTX),
					atomic.LoadUint64(&c.udpRX), atomic.LoadUint64(&c.udpTX))
			default:
			}
		}
	}

	return fmt.Errorf("commonOnce: context error: %w", c.ctx.Err())
}

func (c *Common) outgoingVerify(signal Signal) {
	for c.ctx.Err() == nil {
		if c.tunnelPool.Ready() {
			break
		}
		select {
		case <-c.ctx.Done():
			continue
		case <-time.After(contextCheckInterval):
		}
	}

	fingerPrint := signal.Fingerprint
	if fingerPrint == "" {
		c.logger.Error("outgoingVerify: no fingerprint in signal")
		c.cancel()
		return
	}

	id := signal.PoolConnID
	c.logger.Debug("TLS verify signal: cid %v <- %v", id, c.controlConn.RemoteAddr())

	testConn, err := c.tunnelPool.OutgoingGet(id, poolGetTimeout)
	if err != nil {
		c.logger.Error("outgoingVerify: request timeout: %v", err)
		c.cancel()
		return
	}
	defer testConn.Close()

	var serverFingerprint, clientFingerprint string
	switch c.coreType {
	case "server":
		if c.tlsConfig == nil || len(c.tlsConfig.Certificates) == 0 {
			c.logger.Error("outgoingVerify: no local certificate")
			c.cancel()
			return
		}

		cert := c.tlsConfig.Certificates[0]
		if len(cert.Certificate) == 0 {
			c.logger.Error("outgoingVerify: empty local certificate")
			c.cancel()
			return
		}

		serverFingerprint = c.formatCertFingerprint(cert.Certificate[0])
		clientFingerprint = fingerPrint
	case "client":
		conn, ok := testConn.(interface{ ConnectionState() tls.ConnectionState })
		if !ok {
			return
		}
		state := conn.ConnectionState()

		if len(state.PeerCertificates) == 0 {
			c.logger.Error("outgoingVerify: no peer certificates found")
			c.cancel()
			return
		}

		clientFingerprint = c.formatCertFingerprint(state.PeerCertificates[0].Raw)
		serverFingerprint = fingerPrint
	}

	if serverFingerprint != clientFingerprint {
		c.logger.Error("outgoingVerify: certificate fingerprint mismatch: server: %v - client: %v", serverFingerprint, clientFingerprint)
		c.cancel()
		return
	}

	c.logger.Info("TLS code-1: RAM cert fingerprint verified: %v", fingerPrint)

	c.verifyChan <- struct{}{}
}

func (c *Common) commonTCPOnce(signal Signal) {
	id := signal.PoolConnID
	c.logger.Debug("TCP launch signal: cid %v <- %v", id, c.controlConn.RemoteAddr())

	remoteConn, err := c.tunnelPool.OutgoingGet(id, poolGetTimeout)
	if err != nil {
		c.logger.Error("commonTCPOnce: request timeout: %v", err)
		c.tunnelPool.AddError()
		return
	}

	c.logger.Debug("Tunnel connection: get %v <- pool active %v", id, c.tunnelPool.Active())

	defer func() {
		if remoteConn != nil {
			remoteConn.Close()
			c.logger.Debug("Tunnel connection: closed %v", id)
		}
	}()

	c.logger.Debug("Tunnel connection: %v <-> %v", remoteConn.LocalAddr(), remoteConn.RemoteAddr())

	if !c.tryAcquireSlot(false) {
		c.logger.Error("commonTCPOnce: TCP slot limit reached: %v/%v", c.tcpSlot, c.slotLimit)
		return
	}

	defer c.releaseSlot(false)

	targetConn, err := c.dialWithRotation("tcp", tcpDialTimeout)
	if err != nil {
		c.logger.Error("commonTCPOnce: dialWithRotation failed: %v", err)
		return
	}

	defer func() {
		if targetConn != nil {
			targetConn.Close()
		}
	}()

	targetConn = &conn.StatConn{Conn: targetConn, RX: &c.tcpRX, TX: &c.tcpTX, Rate: c.rateLimiter}
	c.logger.Debug("Target connection: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())

	if err := c.sendProxyV1Header(signal.RemoteAddr, targetConn); err != nil {
		c.logger.Error("commonTCPOnce: sendProxyV1Header failed: %v", err)
		return
	}

	buffer1 := c.getTCPBuffer()
	buffer2 := c.getTCPBuffer()
	defer func() {
		c.putTCPBuffer(buffer1)
		c.putTCPBuffer(buffer2)
	}()

	c.logger.Info("Starting exchange: %v <-> %v", remoteConn.RemoteAddr(), targetConn.RemoteAddr())
	c.logger.Info("Exchange complete: %v", conn.DataExchange(remoteConn, targetConn, c.readTimeout, buffer1, buffer2))
}

func (c *Common) commonUDPOnce(signal Signal) {
	id := signal.PoolConnID
	c.logger.Debug("UDP launch signal: cid %v <- %v", id, c.controlConn.RemoteAddr())

	remoteConn, err := c.tunnelPool.OutgoingGet(id, poolGetTimeout)
	if err != nil {
		c.logger.Error("commonUDPOnce: request timeout: %v", err)
		c.tunnelPool.AddError()
		return
	}

	c.logger.Debug("Tunnel connection: get %v <- pool active %v", id, c.tunnelPool.Active())
	c.logger.Debug("Tunnel connection: %v <-> %v", remoteConn.LocalAddr(), remoteConn.RemoteAddr())

	defer func() {
		if remoteConn != nil {
			remoteConn.Close()
			c.logger.Debug("Tunnel connection: closed %v", id)
		}
	}()

	var targetConn net.Conn
	sessionKey := signal.RemoteAddr
	isNewSession := false

	if session, ok := c.targetUDPSession.Load(sessionKey); ok {
		targetConn = session.(net.Conn)
		c.logger.Debug("Using UDP session: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())
	} else {
		isNewSession = true

		if !c.tryAcquireSlot(true) {
			c.logger.Error("commonUDPOnce: UDP slot limit reached: %v/%v", c.udpSlot, c.slotLimit)
			return
		}

		newSession, err := c.dialWithRotation("udp", udpDialTimeout)
		if err != nil {
			c.logger.Error("commonUDPOnce: dialWithRotation failed: %v", err)
			c.releaseSlot(true)
			return
		}
		targetConn = &conn.StatConn{Conn: newSession, RX: &c.udpRX, TX: &c.udpTX, Rate: c.rateLimiter}
		c.targetUDPSession.Store(sessionKey, targetConn)
		c.logger.Debug("Target connection: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())
	}

	if isNewSession {
		defer func() {
			c.targetUDPSession.Delete(sessionKey)
			if targetConn != nil {
				targetConn.Close()
			}
			c.releaseSlot(true)
		}()
	}

	c.logger.Debug("Starting transfer: %v <-> %v", remoteConn.LocalAddr(), targetConn.LocalAddr())

	done := make(chan struct{}, 2)

	go func() {
		defer func() { done <- struct{}{} }()

		buffer := c.getUDPBuffer()
		defer c.putUDPBuffer(buffer)
		reader := &conn.TimeoutReader{Conn: remoteConn, Timeout: udpReadTimeout}

		for c.ctx.Err() == nil {
			x, err := reader.Read(buffer)
			if err != nil {
				if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
					c.logger.Debug("UDP session abort: %v", err)
				} else if err != io.EOF {
					c.logger.Error("commonUDPOnce: read from tunnel failed: %v", err)
				}
				return
			}

			_, err = targetConn.Write(buffer[:x])
			if err != nil {
				if err != io.EOF {
					c.logger.Error("commonUDPOnce: write to target failed: %v", err)
				}
				return
			}

			c.logger.Debug("Transfer complete: %v <-> %v", remoteConn.LocalAddr(), targetConn.LocalAddr())
		}
	}()

	go func() {
		defer func() { done <- struct{}{} }()

		buffer := c.getUDPBuffer()
		defer c.putUDPBuffer(buffer)
		reader := &conn.TimeoutReader{Conn: targetConn, Timeout: udpReadTimeout}

		for c.ctx.Err() == nil {
			x, err := reader.Read(buffer)
			if err != nil {
				if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
					c.logger.Debug("UDP session abort: %v", err)
				} else if err != io.EOF {
					c.logger.Error("commonUDPOnce: read from target failed: %v", err)
				}
				return
			}

			_, err = remoteConn.Write(buffer[:x])
			if err != nil {
				if err != io.EOF {
					c.logger.Error("commonUDPOnce: write to tunnel failed: %v", err)
				}
				return
			}

			c.logger.Debug("Transfer complete: %v <-> %v", targetConn.LocalAddr(), remoteConn.LocalAddr())
		}
	}()

	<-done
}

func (c *Common) singleControl() error {
	errChan := make(chan error, 3)

	if len(c.targetTCPAddrs) > 0 {
		go func() { errChan <- c.singleEventLoop() }()
	}
	if c.tunnelListener != nil || c.disableTCP != "1" {
		go func() { errChan <- c.singleTCPLoop() }()
	}
	if c.tunnelUDPConn != nil || c.disableUDP != "1" {
		go func() { errChan <- c.singleUDPLoop() }()
	}

	select {
	case <-c.ctx.Done():
		return fmt.Errorf("singleControl: context error: %w", c.ctx.Err())
	case err := <-errChan:
		return fmt.Errorf("singleControl: %w", err)
	}
}

func (c *Common) singleEventLoop() error {
	ticker := time.NewTicker(reportInterval)
	defer ticker.Stop()

	for c.ctx.Err() == nil {
		c.logger.Event("CHECK_POINT|MODE=%v|PING=%vms|POOL=0|TCPS=%v|UDPS=%v|TCPRX=%v|TCPTX=%v|UDPRX=%v|UDPTX=%v", c.runMode, c.probeBestTarget(),
			atomic.LoadInt32(&c.tcpSlot), atomic.LoadInt32(&c.udpSlot),
			atomic.LoadUint64(&c.tcpRX), atomic.LoadUint64(&c.tcpTX),
			atomic.LoadUint64(&c.udpRX), atomic.LoadUint64(&c.udpTX))

		select {
		case <-c.ctx.Done():
			return fmt.Errorf("singleEventLoop: context error: %w", c.ctx.Err())
		case <-ticker.C:
		}
	}

	return fmt.Errorf("singleEventLoop: context error: %w", c.ctx.Err())
}

func (c *Common) singleTCPLoop() error {
	for c.ctx.Err() == nil {
		tunnelConn, err := c.tunnelListener.Accept()
		if err != nil {
			if c.ctx.Err() != nil || err == net.ErrClosed {
				return fmt.Errorf("singleTCPLoop: context error: %w", c.ctx.Err())
			}
			c.logger.Error("singleTCPLoop: accept failed: %v", err)

			select {
			case <-c.ctx.Done():
				return fmt.Errorf("singleTCPLoop: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		tunnelConn = &conn.StatConn{Conn: tunnelConn, RX: &c.tcpRX, TX: &c.tcpTX, Rate: c.rateLimiter}
		c.logger.Debug("Tunnel connection: %v <-> %v", tunnelConn.LocalAddr(), tunnelConn.RemoteAddr())

		go func(tunnelConn net.Conn) {
			defer func() {
				if tunnelConn != nil {
					tunnelConn.Close()
				}
			}()

			if !c.tryAcquireSlot(false) {
				c.logger.Error("singleTCPLoop: TCP slot limit reached: %v/%v", c.tcpSlot, c.slotLimit)
				return
			}

			defer c.releaseSlot(false)

			protocol, wrappedConn := c.detectBlockProtocol(tunnelConn)
			if protocol != "" {
				c.logger.Warn("singleTCPLoop: blocked %v protocol from %v", protocol, tunnelConn.RemoteAddr())
				return
			}
			tunnelConn = wrappedConn

			targetConn, err := c.dialWithRotation("tcp", tcpDialTimeout)
			if err != nil {
				c.logger.Error("singleTCPLoop: dialWithRotation failed: %v", err)
				return
			}

			defer func() {
				if targetConn != nil {
					targetConn.Close()
				}
			}()

			c.logger.Debug("Target connection: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())

			if err := c.sendProxyV1Header(tunnelConn.RemoteAddr().String(), targetConn); err != nil {
				c.logger.Error("singleTCPLoop: sendProxyV1Header failed: %v", err)
				return
			}

			buffer1 := c.getTCPBuffer()
			buffer2 := c.getTCPBuffer()
			defer func() {
				c.putTCPBuffer(buffer1)
				c.putTCPBuffer(buffer2)
			}()

			c.logger.Info("Starting exchange: %v <-> %v", tunnelConn.RemoteAddr(), targetConn.RemoteAddr())
			c.logger.Info("Exchange complete: %v", conn.DataExchange(tunnelConn, targetConn, c.readTimeout, buffer1, buffer2))
		}(tunnelConn)
	}

	return fmt.Errorf("singleTCPLoop: context error: %w", c.ctx.Err())
}

func (c *Common) singleUDPLoop() error {
	for c.ctx.Err() == nil {
		buffer := c.getUDPBuffer()

		x, clientAddr, err := c.tunnelUDPConn.ReadFromUDP(buffer)
		if err != nil {
			if c.ctx.Err() != nil || err == net.ErrClosed {
				c.putUDPBuffer(buffer)
				return fmt.Errorf("singleUDPLoop: context error: %w", c.ctx.Err())
			}
			c.logger.Error("singleUDPLoop: ReadFromUDP failed: %v", err)

			c.putUDPBuffer(buffer)
			select {
			case <-c.ctx.Done():
				return fmt.Errorf("singleUDPLoop: context error: %w", c.ctx.Err())
			case <-time.After(contextCheckInterval):
			}
			continue
		}

		c.logger.Debug("Tunnel connection: %v <-> %v", c.tunnelUDPConn.LocalAddr(), clientAddr)

		var targetConn net.Conn
		sessionKey := clientAddr.String()

		if session, ok := c.targetUDPSession.Load(sessionKey); ok {
			targetConn = session.(net.Conn)
			c.logger.Debug("Using UDP session: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())
		} else {
			if !c.tryAcquireSlot(true) {
				c.logger.Error("singleUDPLoop: UDP slot limit reached: %v/%v", c.udpSlot, c.slotLimit)
				c.putUDPBuffer(buffer)
				continue
			}

			newSession, err := c.dialWithRotation("udp", udpDialTimeout)
			if err != nil {
				c.logger.Error("singleUDPLoop: dialWithRotation failed: %v", err)
				c.releaseSlot(true)
				c.putUDPBuffer(buffer)
				continue
			}
			targetConn = newSession
			c.targetUDPSession.Store(sessionKey, newSession)
			c.logger.Debug("Target connection: %v <-> %v", targetConn.LocalAddr(), targetConn.RemoteAddr())

			go func(targetConn net.Conn, clientAddr *net.UDPAddr, sessionKey string) {
				defer func() {
					if targetConn != nil {
						targetConn.Close()
					}
					c.releaseSlot(true)
				}()

				buffer := c.getUDPBuffer()
				defer c.putUDPBuffer(buffer)
				reader := &conn.TimeoutReader{Conn: targetConn, Timeout: udpReadTimeout}

				for c.ctx.Err() == nil {
					x, err := reader.Read(buffer)
					if err != nil {
						if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
							c.logger.Debug("UDP session abort: %v", err)
						} else if err != io.EOF {
							c.logger.Error("singleUDPLoop: read from target failed: %v", err)
						}
						c.targetUDPSession.Delete(sessionKey)
						if targetConn != nil {
							targetConn.Close()
						}
						return
					}

					_, err = c.tunnelUDPConn.WriteToUDP(buffer[:x], clientAddr)
					if err != nil {
						if err != io.EOF {
							c.logger.Error("singleUDPLoop: writeToUDP failed: %v", err)
						}
						c.targetUDPSession.Delete(sessionKey)
						if targetConn != nil {
							targetConn.Close()
						}
						return
					}
					c.logger.Debug("Transfer complete: %v <-> %v", c.tunnelUDPConn.LocalAddr(), targetConn.LocalAddr())
				}
			}(targetConn, clientAddr, sessionKey)
		}

		c.logger.Debug("Starting transfer: %v <-> %v", targetConn.LocalAddr(), c.tunnelUDPConn.LocalAddr())
		_, err = targetConn.Write(buffer[:x])
		if err != nil {
			if err != io.EOF {
				c.logger.Error("singleUDPLoop: write to target failed: %v", err)
			}
			c.targetUDPSession.Delete(sessionKey)
			if targetConn != nil {
				targetConn.Close()
			}
			c.putUDPBuffer(buffer)
			return fmt.Errorf("singleUDPLoop: write to target failed: %w", err)
		}

		c.logger.Debug("Transfer complete: %v <-> %v", targetConn.LocalAddr(), c.tunnelUDPConn.LocalAddr())
		c.putUDPBuffer(buffer)
	}

	return fmt.Errorf("singleUDPLoop: context error: %w", c.ctx.Err())
}
