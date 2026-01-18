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
	"sync"
	"syscall"
	"time"

	"github.com/NodePassProject/logs"
	"github.com/NodePassProject/nph2"
	"github.com/NodePassProject/npws"
	"github.com/NodePassProject/pool"
	"github.com/NodePassProject/quic"
)

type Client struct{ Common }

func NewClient(parsedURL *url.URL, logger *logs.Logger) (*Client, error) {
	client := &Client{
		Common: Common{
			parsedURL:  parsedURL,
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
	if err := client.initConfig(); err != nil {
		return nil, fmt.Errorf("newClient: initConfig failed: %w", err)
	}
	client.initRateLimiter()
	return client, nil
}

func (c *Client) Run() {
	logInfo := func(prefix string) {
		c.logger.Info("%v: client://%v@%v/%v?dns=%v&sni=%v&lbs=%v&min=%v&mode=%v&dial=%v&read=%v&rate=%v&slot=%v&proxy=%v&block=%v&notcp=%v&noudp=%v",
			prefix, c.tunnelKey, c.tunnelTCPAddr, c.getTargetAddrsString(), c.dnsCacheTTL, c.serverName, c.lbStrategy, c.minPoolCapacity,
			c.runMode, c.dialerIP, c.readTimeout, c.rateLimit/125000, c.slotLimit,
			c.proxyProtocol, c.blockProtocol, c.disableTCP, c.disableUDP)
	}
	logInfo("Client started")

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)

	go func() {
		for ctx.Err() == nil {
			if err := c.start(); err != nil && err != io.EOF {
				c.logger.Error("Client error: %v", err)
				c.stop()
				select {
				case <-ctx.Done():
					return
				case <-time.After(serviceCooldown):
				}
				logInfo("Client restart")
			}
		}
	}()

	<-ctx.Done()
	stop()

	shutdownCtx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
	defer cancel()
	if err := c.shutdown(shutdownCtx, c.stop); err != nil {
		c.logger.Error("Client shutdown error: %v", err)
	} else {
		c.logger.Info("Client shutdown complete")
	}
}

func (c *Client) start() error {
	c.initContext()

	switch c.runMode {
	case "1":
		if err := c.initTunnelListener(); err == nil {
			return c.singleStart()
		} else {
			return fmt.Errorf("start: initTunnelListener failed: %w", err)
		}
	case "2":
		return c.commonStart()
	default:
		if err := c.initTunnelListener(); err == nil {
			c.runMode = "1"
			return c.singleStart()
		} else {
			c.runMode = "2"
			return c.commonStart()
		}
	}
}

func (c *Client) singleStart() error {
	if err := c.singleControl(); err != nil {
		return fmt.Errorf("singleStart: singleControl failed: %w", err)
	}
	return nil
}

func (c *Client) commonStart() error {
	c.logger.Info("Pending tunnel handshake...")
	c.handshakeStart = time.Now()
	if err := c.tunnelHandshake(); err != nil {
		return fmt.Errorf("commonStart: tunnelHandshake failed: %w", err)
	}

	if err := c.initTunnelPool(); err != nil {
		return fmt.Errorf("commonStart: initTunnelPool failed: %w", err)
	}

	c.logger.Info("Getting tunnel pool ready...")
	if err := c.setControlConn(); err != nil {
		return fmt.Errorf("commonStart: setControlConn failed: %w", err)
	}

	if c.dataFlow == "+" {
		if err := c.initTargetListener(); err != nil {
			return fmt.Errorf("commonStart: initTargetListener failed: %w", err)
		}
		go c.commonLoop()
	}

	if err := c.commonControl(); err != nil {
		return fmt.Errorf("commonStart: commonControl failed: %w", err)
	}

	return nil
}

func (c *Client) initTunnelPool() error {
	switch c.poolType {
	case "0":
		tcpPool := pool.NewClientPool(
			c.minPoolCapacity,
			c.maxPoolCapacity,
			minPoolInterval,
			maxPoolInterval,
			reportInterval,
			c.tlsCode,
			c.serverName,
			func() (net.Conn, error) {
				tcpAddr, err := c.getTunnelTCPAddr()
				if err != nil {
					return nil, err
				}
				return net.DialTimeout("tcp", tcpAddr.String(), tcpDialTimeout)
			})
		go tcpPool.ClientManager()
		c.tunnelPool = tcpPool
	case "1":
		quicPool := quic.NewClientPool(
			c.minPoolCapacity,
			c.maxPoolCapacity,
			minPoolInterval,
			maxPoolInterval,
			reportInterval,
			c.tlsCode,
			c.serverName,
			func() (string, error) {
				udpAddr, err := c.getTunnelUDPAddr()
				if err != nil {
					return "", err
				}
				return udpAddr.String(), nil
			})
		go quicPool.ClientManager()
		c.tunnelPool = quicPool
	case "2":
		websocketPool := npws.NewClientPool(
			c.minPoolCapacity,
			c.maxPoolCapacity,
			minPoolInterval,
			maxPoolInterval,
			reportInterval,
			c.tlsCode,
			c.tunnelAddr)
		go websocketPool.ClientManager()
		c.tunnelPool = websocketPool
	case "3":
		http2Pool := nph2.NewClientPool(
			c.minPoolCapacity,
			c.maxPoolCapacity,
			minPoolInterval,
			maxPoolInterval,
			reportInterval,
			c.tlsCode,
			c.serverName,
			func() (string, error) {
				tcpAddr, err := c.getTunnelTCPAddr()
				if err != nil {
					return "", err
				}
				return tcpAddr.String(), nil
			})
		go http2Pool.ClientManager()
		c.tunnelPool = http2Pool
	default:
		return fmt.Errorf("initTunnelPool: unknown pool type: %s", c.poolType)
	}
	return nil
}

func (c *Client) tunnelHandshake() error {
	req, _ := http.NewRequest(http.MethodGet, "https://"+c.tunnelAddr+"/", nil)
	req.Host = c.serverName
	req.Header.Set("Authorization", "Bearer "+c.generateAuthToken())

	client := &http.Client{
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{
				InsecureSkipVerify: true,
			},
		},
	}

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("tunnelHandshake: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("tunnelHandshake: status %d", resp.StatusCode)
	}

	var config struct {
		Flow string `json:"flow"`
		Max  int    `json:"max"`
		TLS  string `json:"tls"`
		Type string `json:"type"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&config); err != nil {
		return fmt.Errorf("tunnelHandshake: %w", err)
	}

	c.dataFlow = config.Flow
	c.maxPoolCapacity = config.Max
	c.tlsCode = config.TLS
	c.poolType = config.Type

	c.logger.Info("Loading tunnel config: FLOW=%v|MAX=%v|TLS=%v|TYPE=%v",
		c.dataFlow, c.maxPoolCapacity, c.tlsCode, c.poolType)
	return nil
}
