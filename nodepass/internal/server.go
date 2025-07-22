// 内部包，实现服务端模式功能
package internal

import (
	"bufio"
	"bytes"
	"context"
	"crypto/tls"
	"net"
	"net/url"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/NodePassProject/conn"
	"github.com/NodePassProject/logs"
	"github.com/NodePassProject/pool"
)

// Server 实现服务端模式功能
type Server struct {
	Common                // 继承共享功能
	tlsConfig *tls.Config // TLS配置
	clientIP  string      // 客户端IP
}

// NewServer 创建新的服务端实例
func NewServer(parsedURL *url.URL, tlsCode string, tlsConfig *tls.Config, logger *logs.Logger) *Server {
	server := &Server{
		Common: Common{
			tlsCode:    tlsCode,
			dataFlow:   "+",
			logger:     logger,
			semaphore:  make(chan struct{}, semaphoreLimit),
			signalChan: make(chan string, semaphoreLimit),
		},
		tlsConfig: tlsConfig,
	}
	// 初始化公共字段
	server.getTunnelKey(parsedURL)
	server.getPoolCapacity(parsedURL)
	server.getAddress(parsedURL)
	return server
}

// Run 管理服务端生命周期
func (s *Server) Run() {
	s.logger.Info("Server started: %v@%v/%v", s.tunnelKey, s.tunnelAddr, s.targetTCPAddr)

	// 启动服务端并处理重启
	go func() {
		for {
			time.Sleep(serviceCooldown)
			if err := s.start(); err != nil {
				s.logger.Error("Server error: %v", err)
				s.stop()
				s.logger.Info("Server restarted: %v@%v/%v", s.tunnelKey, s.tunnelAddr, s.targetTCPAddr)
			}
		}
	}()

	// 监听系统信号以优雅关闭
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	<-ctx.Done()
	stop()

	// 执行关闭过程
	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := s.shutdown(shutdownCtx, s.stop); err != nil {
		s.logger.Error("Server shutdown error: %v", err)
	} else {
		s.logger.Info("Server shutdown complete")
	}
}

// start 启动服务端
func (s *Server) start() error {
	// 初始化基本信息
	s.initBackground()

	// 初始化隧道监听器
	if err := s.initTunnelListener(); err != nil {
		return err
	}

	// 通过是否监听成功判断数据流向
	if err := s.initTargetListener(); err == nil {
		s.dataFlow = "-"
	}

	// 与客户端进行握手
	if err := s.tunnelHandshake(); err != nil {
		return err
	}

	// 握手之后把UDP监听关掉
	if s.tunnelUDPConn != nil {
		s.tunnelUDPConn.Close()
	}

	// 初始化隧道连接池
	s.tunnelPool = pool.NewServerPool(
		s.maxPoolCapacity,
		s.clientIP,
		s.tlsConfig,
		s.tunnelListener,
		reportInterval)

	go s.tunnelPool.ServerManager()

	if s.dataFlow == "-" {
		go s.commonLoop()
	}

	return s.commonControl()
}

// tunnelHandshake 与客户端进行握手
func (s *Server) tunnelHandshake() error {
	// 接受隧道连接
	for {
		tunnelTCPConn, err := s.tunnelListener.Accept()
		if err != nil {
			s.logger.Error("Accept error: %v", err)
			time.Sleep(serviceCooldown)
			continue
		}

		tunnelTCPConn.SetReadDeadline(time.Now().Add(tcpReadTimeout))

		bufReader := bufio.NewReader(tunnelTCPConn)
		rawTunnelKey, err := bufReader.ReadString('\n')
		if err != nil {
			s.logger.Warn("Handshake timeout: %v", tunnelTCPConn.RemoteAddr())
			tunnelTCPConn.Close()
			time.Sleep(serviceCooldown)
			continue
		}

		tunnelTCPConn.SetReadDeadline(time.Time{})
		tunnelKey := string(s.xor(bytes.TrimSuffix([]byte(rawTunnelKey), []byte{'\n'})))

		if tunnelKey != s.tunnelKey {
			s.logger.Warn("Access denied: %v", tunnelTCPConn.RemoteAddr())
			tunnelTCPConn.Close()
			time.Sleep(serviceCooldown)
			continue
		} else {
			s.tunnelTCPConn = tunnelTCPConn.(*net.TCPConn)
			s.bufReader = bufio.NewReader(&conn.TimeoutReader{Conn: s.tunnelTCPConn, Timeout: tcpReadTimeout})
			s.tunnelTCPConn.SetKeepAlive(true)
			s.tunnelTCPConn.SetKeepAlivePeriod(reportInterval)

			// 记录客户端IP
			s.clientIP = s.tunnelTCPConn.RemoteAddr().(*net.TCPAddr).IP.String()
			break
		}
	}

	// 构建并发送隧道URL到客户端
	tunnelURL := &url.URL{
		Host:     s.dataFlow,
		Fragment: s.tlsCode,
	}

	_, err := s.tunnelTCPConn.Write(append(s.xor([]byte(tunnelURL.String())), '\n'))
	if err != nil {
		return err
	}

	s.logger.Info("Tunnel signal -> : %v -> %v", tunnelURL.String(), s.tunnelTCPConn.RemoteAddr())
	s.logger.Info("Tunnel handshaked: %v <-> %v", s.tunnelTCPConn.LocalAddr(), s.tunnelTCPConn.RemoteAddr())
	return nil
}
