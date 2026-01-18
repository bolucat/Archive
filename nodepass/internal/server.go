package internal

import (
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/NodePassProject/logs"
	"github.com/NodePassProject/nph2"
	"github.com/NodePassProject/npws"
	"github.com/NodePassProject/pool"
	"github.com/NodePassProject/quic"
)

type Server struct{ Common }

func NewServer(parsedURL *url.URL, tlsCode string, tlsConfig *tls.Config, logger *logs.Logger) (*Server, error) {
	server := &Server{
		Common: Common{
			parsedURL:  parsedURL,
			tlsCode:    tlsCode,
			tlsConfig:  tlsConfig,
			logger:     logger,
			signalChan: make(chan Signal, semaphoreLimit),
			writeChan:  make(chan []byte, semaphoreLimit),
			verifyChan: make(chan struct{}),
			tcpBufferPool: &sync.Pool{
				New: func() any {
					buf := make([]byte, tcpDataBufSize)
					return &buf
				},
			},
			udpBufferPool: &sync.Pool{
				New: func() any {
					buf := make([]byte, udpDataBufSize)
					return &buf
				},
			},
		},
	}
	if err := server.initConfig(); err != nil {
		return nil, fmt.Errorf("newServer: initConfig failed: %w", err)
	}
	server.initRateLimiter()
	return server, nil
}

func (s *Server) Run() {
	logInfo := func(prefix string) {
		s.logger.Info("%v: server://%v@%v/%v?dns=%v&lbs=%v&max=%v&mode=%v&type=%v&dial=%v&read=%v&rate=%v&slot=%v&proxy=%v&block=%v&notcp=%v&noudp=%v",
			prefix, s.tunnelKey, s.tunnelTCPAddr, s.getTargetAddrsString(), s.dnsCacheTTL, s.lbStrategy, s.maxPoolCapacity,
			s.runMode, s.poolType, s.dialerIP, s.readTimeout, s.rateLimit/125000, s.slotLimit,
			s.proxyProtocol, s.blockProtocol, s.disableTCP, s.disableUDP)
	}
	logInfo("Server started")

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)

	go func() {
		for ctx.Err() == nil {
			if err := s.start(); err != nil && err != io.EOF {
				s.logger.Error("Server error: %v", err)
				s.stop()
				select {
				case <-ctx.Done():
					return
				case <-time.After(serviceCooldown):
				}
				logInfo("Server restart")
			}
		}
	}()

	<-ctx.Done()
	stop()

	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := s.shutdown(shutdownCtx, s.stop); err != nil {
		s.logger.Error("Server shutdown error: %v", err)
	} else {
		s.logger.Info("Server shutdown complete")
	}
}

func (s *Server) start() error {
	s.initContext()

	if err := s.initTunnelListener(); err != nil {
		return fmt.Errorf("start: initTunnelListener failed: %w", err)
	}

	if s.tunnelUDPConn != nil {
		s.tunnelUDPConn.Close()
	}

	switch s.runMode {
	case "1":
		if err := s.initTargetListener(); err != nil {
			return fmt.Errorf("start: initTargetListener failed: %w", err)
		}
		s.dataFlow = "-"
	case "2":
		s.dataFlow = "+"
	default:
		if err := s.initTargetListener(); err == nil {
			s.runMode = "1"
			s.dataFlow = "-"
		} else {
			s.runMode = "2"
			s.dataFlow = "+"
		}
	}

	s.logger.Info("Pending tunnel handshake...")
	s.handshakeStart = time.Now()
	if err := s.tunnelHandshake(); err != nil {
		return fmt.Errorf("start: tunnelHandshake failed: %w", err)
	}

	if err := s.initTunnelPool(); err != nil {
		return fmt.Errorf("start: initTunnelPool failed: %w", err)
	}

	s.logger.Info("Getting tunnel pool ready...")
	if err := s.setControlConn(); err != nil {
		return fmt.Errorf("start: setControlConn failed: %w", err)
	}

	if s.dataFlow == "-" {
		go s.commonLoop()
	}

	if err := s.commonControl(); err != nil {
		return fmt.Errorf("start: commonControl failed: %w", err)
	}
	return nil
}

func (s *Server) initTunnelPool() error {
	switch s.poolType {
	case "0":
		tcpPool := pool.NewServerPool(
			s.maxPoolCapacity,
			s.clientIP,
			s.tlsConfig,
			s.tunnelListener,
			reportInterval)
		go tcpPool.ServerManager()
		s.tunnelPool = tcpPool
	case "1":
		quicPool := quic.NewServerPool(
			s.maxPoolCapacity,
			s.clientIP,
			s.tlsConfig,
			s.tunnelUDPAddr.String(),
			reportInterval)
		go quicPool.ServerManager()
		s.tunnelPool = quicPool
	case "2":
		websocketPool := npws.NewServerPool(
			s.maxPoolCapacity,
			"",
			s.tlsConfig,
			s.tunnelListener,
			reportInterval)
		go websocketPool.ServerManager()
		s.tunnelPool = websocketPool
	case "3":
		http2Pool := nph2.NewServerPool(
			s.maxPoolCapacity,
			s.clientIP,
			s.tlsConfig,
			s.tunnelListener,
			reportInterval)
		go http2Pool.ServerManager()
		s.tunnelPool = http2Pool
	default:
		return fmt.Errorf("initTunnelPool: unknown pool type: %s", s.poolType)
	}
	return nil
}

func (s *Server) tunnelHandshake() error {
	var clientIP string
	done := make(chan struct{})

	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Connection", "close")
		if r.Method != http.MethodGet {
			http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
			return
		}

		if r.URL.Path != "/" {
			http.Error(w, "Not Found", http.StatusNotFound)
			return
		}

		auth := r.Header.Get("Authorization")
		if !strings.HasPrefix(auth, "Bearer ") || !s.verifyAuthToken(strings.TrimPrefix(auth, "Bearer ")) {
			http.Error(w, "Unauthorized", http.StatusUnauthorized)
			return
		}

		clientIP = r.RemoteAddr
		if host, _, err := net.SplitHostPort(clientIP); err == nil {
			clientIP = host
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"flow": s.dataFlow,
			"max":  s.maxPoolCapacity,
			"tls":  s.tlsCode,
			"type": s.poolType,
		})

		s.logger.Info("Sending tunnel config: FLOW=%v|MAX=%v|TLS=%v|TYPE=%v",
			s.dataFlow, s.maxPoolCapacity, s.tlsCode, s.poolType)

		close(done)
	})

	tlsConfig := s.tlsConfig
	if tlsConfig == nil {
		tlsConfig, _ = NewTLSConfig()
	}

	server := &http.Server{
		Handler:   handler,
		TLSConfig: tlsConfig,
		ErrorLog:  s.logger.StdLogger(),
	}
	go server.ServeTLS(s.tunnelListener, "", "")

	select {
	case <-done:
		server.Close()
		s.clientIP = clientIP
		if s.tlsCode == "1" {
			if newTLSConfig, err := NewTLSConfig(); err == nil {
				newTLSConfig.MinVersion = tls.VersionTLS13
				s.tlsConfig = newTLSConfig
				s.logger.Info("TLS code-1: RAM cert regenerated with TLS 1.3")
			} else {
				s.logger.Warn("Failed to regenerate RAM cert: %v", err)
			}
		}

		s.tunnelListener, _ = net.ListenTCP("tcp", s.tunnelTCPAddr)
		return nil
	case <-s.ctx.Done():
		server.Close()
		return fmt.Errorf("tunnelHandshake: context canceled")
	}
}
