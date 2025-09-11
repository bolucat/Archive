// 内部包，实现服务端模式功能
package internal

import (
	"bufio"
	"bytes"
	"context"
	"crypto/tls"
	"fmt"
	"io"
	"net"
	"net/url"
	"os"
	"os/signal"
	"strconv"
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
			logger:     logger,
			signalChan: make(chan string, semaphoreLimit),
		},
		tlsConfig: tlsConfig,
	}
	server.initConfig(parsedURL)
	server.initRateLimiter()
	return server
}

// Run 管理服务端生命周期
func (s *Server) Run() {
	logInfo := func(prefix string) {
		s.logger.Info("%v: %v@%v/%v?max=%v&mode=%v&read=%v&rate=%v&slot=%v",
			prefix, s.tunnelKey, s.tunnelTCPAddr, s.targetTCPAddr,
			s.maxPoolCapacity, s.runMode, s.readTimeout, s.rateLimit/125000, s.slotLimit)
	}
	logInfo("Server started")

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)

	// 启动服务端并处理重启
	go func() {
		for {
			if ctx.Err() != nil {
				return
			}
			// 启动服务端
			if err := s.start(); err != nil && err != io.EOF {
				s.logger.Error("Server error: %v", err)
				// 重启服务端
				s.stop()
				select {
				case <-ctx.Done():
					return
				case <-time.After(serviceCooldown):
				}
				logInfo("Server restarting")
			}
		}
	}()

	// 监听系统信号以优雅关闭
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
	// 初始化上下文
	s.initContext()

	// 初始化隧道监听器
	if err := s.initTunnelListener(); err != nil {
		return fmt.Errorf("start: initTunnelListener failed: %w", err)
	}

	// 运行模式判断
	switch s.runMode {
	case "1": // 反向模式
		if err := s.initTargetListener(); err != nil {
			return fmt.Errorf("start: initTargetListener failed: %w", err)
		}
		s.dataFlow = "-"
	case "2": // 正向模式
		s.dataFlow = "+"
	default: // 自动判断
		if err := s.initTargetListener(); err == nil {
			s.runMode = "1"
			s.dataFlow = "-"
		} else {
			s.runMode = "2"
			s.dataFlow = "+"
		}
	}

	// 与客户端进行握手
	if err := s.tunnelHandshake(); err != nil {
		return fmt.Errorf("start: tunnelHandshake failed: %w", err)
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
	if err := s.commonControl(); err != nil {
		return fmt.Errorf("start: commonControl failed: %w", err)
	}
	return nil
}

// tunnelHandshake 与客户端进行握手
func (s *Server) tunnelHandshake() error {
	// 接受隧道连接
	for {
		if s.ctx.Err() != nil {
			return fmt.Errorf("tunnelHandshake: context error: %w", s.ctx.Err())
		}

		tunnelTCPConn, err := s.tunnelListener.Accept()
		if err != nil {
			s.logger.Error("tunnelHandshake: accept error: %v", err)
			select {
			case <-s.ctx.Done():
				return fmt.Errorf("tunnelHandshake: context error: %w", s.ctx.Err())
			case <-time.After(serviceCooldown):
			}
			continue
		}

		tunnelTCPConn.SetReadDeadline(time.Now().Add(handshakeTimeout))

		bufReader := bufio.NewReader(tunnelTCPConn)
		rawTunnelKey, err := bufReader.ReadString('\n')
		if err != nil {
			s.logger.Warn("tunnelHandshake: handshake timeout: %v", tunnelTCPConn.RemoteAddr())
			tunnelTCPConn.Close()
			select {
			case <-s.ctx.Done():
				return fmt.Errorf("tunnelHandshake: context error: %w", s.ctx.Err())
			case <-time.After(serviceCooldown):
			}
			continue
		}

		tunnelTCPConn.SetReadDeadline(time.Time{})
		tunnelKey := string(s.xor(bytes.TrimSuffix([]byte(rawTunnelKey), []byte{'\n'})))

		if tunnelKey != s.tunnelKey {
			s.logger.Warn("tunnelHandshake: access denied: %v", tunnelTCPConn.RemoteAddr())
			tunnelTCPConn.Close()
			select {
			case <-s.ctx.Done():
				return fmt.Errorf("tunnelHandshake: context error: %w", s.ctx.Err())
			case <-time.After(serviceCooldown):
			}
			continue
		}

		s.tunnelTCPConn = tunnelTCPConn.(*net.TCPConn)
		s.bufReader = bufio.NewReader(&conn.TimeoutReader{Conn: s.tunnelTCPConn, Timeout: 2 * reportInterval})
		s.tunnelTCPConn.SetKeepAlive(true)
		s.tunnelTCPConn.SetKeepAlivePeriod(reportInterval)

		// 记录客户端IP
		s.clientIP = s.tunnelTCPConn.RemoteAddr().(*net.TCPAddr).IP.String()
		break
	}

	// 发送客户端配置
	tunnelURL := &url.URL{
		Scheme:   "np",
		Host:     strconv.Itoa(s.maxPoolCapacity),
		Path:     s.dataFlow,
		Fragment: s.tlsCode,
	}

	_, err := s.tunnelTCPConn.Write(append(s.xor([]byte(tunnelURL.String())), '\n'))
	if err != nil {
		return fmt.Errorf("tunnelHandshake: write tunnel config failed: %w", err)
	}

	s.logger.Info("Tunnel signal -> : %v -> %v", tunnelURL.String(), s.tunnelTCPConn.RemoteAddr())
	s.logger.Info("Tunnel handshaked: %v <-> %v", s.tunnelTCPConn.LocalAddr(), s.tunnelTCPConn.RemoteAddr())
	return nil
}
