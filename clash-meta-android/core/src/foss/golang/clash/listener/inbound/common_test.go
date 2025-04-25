package inbound_test

import (
	"context"
	"crypto/rand"
	"crypto/tls"
	"encoding/base64"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"sync"
	"testing"
	"time"

	N "github.com/metacubex/mihomo/common/net"
	"github.com/metacubex/mihomo/common/utils"
	"github.com/metacubex/mihomo/component/ca"
	"github.com/metacubex/mihomo/component/generater"
	C "github.com/metacubex/mihomo/constant"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/render"
	"github.com/stretchr/testify/assert"
)

var httpPath = "/inbound_test"
var httpData = make([]byte, 10240)
var remoteAddr = netip.MustParseAddr("1.2.3.4")
var userUUID = utils.NewUUIDV4().String()
var tlsCertificate, tlsPrivateKey, tlsFingerprint, _ = N.NewRandomTLSKeyPair()
var tlsConfigCert, _ = tls.X509KeyPair([]byte(tlsCertificate), []byte(tlsPrivateKey))
var tlsConfig = &tls.Config{Certificates: []tls.Certificate{tlsConfigCert}, NextProtos: []string{"h2", "http/1.1"}}
var tlsClientConfig, _ = ca.GetTLSConfig(nil, tlsFingerprint, "", "")
var realityPrivateKey, realityPublickey string
var realityDest = "itunes.apple.com"
var realityShortid = "10f897e26c4b9478"

func init() {
	rand.Read(httpData)
	privateKey, err := generater.GeneratePrivateKey()
	if err != nil {
		panic(err)
	}
	publicKey := privateKey.PublicKey()
	realityPrivateKey = base64.RawURLEncoding.EncodeToString(privateKey[:])
	realityPublickey = base64.RawURLEncoding.EncodeToString(publicKey[:])
}

type TestTunnel struct {
	HandleTCPConnFn   func(conn net.Conn, metadata *C.Metadata)
	HandleUDPPacketFn func(packet C.UDPPacket, metadata *C.Metadata)
	NatTableFn        func() C.NatTable
	CloseFn           func() error
	DoTestFn          func(t *testing.T, proxy C.ProxyAdapter)
}

func (tt *TestTunnel) HandleTCPConn(conn net.Conn, metadata *C.Metadata) {
	tt.HandleTCPConnFn(conn, metadata)
}

func (tt *TestTunnel) HandleUDPPacket(packet C.UDPPacket, metadata *C.Metadata) {
	tt.HandleUDPPacketFn(packet, metadata)
}

func (tt *TestTunnel) NatTable() C.NatTable {
	return tt.NatTableFn()
}

func (tt *TestTunnel) Close() error {
	return tt.CloseFn()
}

func (tt *TestTunnel) DoTest(t *testing.T, proxy C.ProxyAdapter) {
	tt.DoTestFn(t, proxy)
}

type TestTunnelListener struct {
	ch     chan net.Conn
	ctx    context.Context
	cancel context.CancelFunc
	addr   net.Addr
}

func (t *TestTunnelListener) Accept() (net.Conn, error) {
	select {
	case conn, ok := <-t.ch:
		if !ok {
			return nil, net.ErrClosed
		}
		return conn, nil
	case <-t.ctx.Done():
		return nil, t.ctx.Err()
	}
}

func (t *TestTunnelListener) Close() error {
	t.cancel()
	return nil
}

func (t *TestTunnelListener) Addr() net.Addr {
	return t.addr
}

type WaitCloseConn struct {
	net.Conn
	ch   chan struct{}
	once sync.Once
}

func (c *WaitCloseConn) Close() error {
	err := c.Conn.Close()
	c.once.Do(func() {
		close(c.ch)
	})
	return err
}

var _ C.Tunnel = (*TestTunnel)(nil)
var _ net.Listener = (*TestTunnelListener)(nil)

func NewHttpTestTunnel() *TestTunnel {
	ctx, cancel := context.WithCancel(context.Background())
	ln := &TestTunnelListener{ch: make(chan net.Conn), ctx: ctx, cancel: cancel, addr: net.TCPAddrFromAddrPort(netip.AddrPortFrom(remoteAddr, 0))}

	r := chi.NewRouter()
	r.Get(httpPath, func(w http.ResponseWriter, r *http.Request) {
		render.Data(w, r, httpData)
	})
	go http.Serve(ln, r)
	testFn := func(t *testing.T, proxy C.ProxyAdapter, proto string) {
		req, err := http.NewRequest(http.MethodGet, fmt.Sprintf("%s://%s%s", proto, remoteAddr, httpPath), nil)
		if !assert.NoError(t, err) {
			return
		}
		req = req.WithContext(ctx)

		var dstPort uint16 = 80
		if proto == "https" {
			dstPort = 443
		}
		metadata := &C.Metadata{
			NetWork: C.TCP,
			DstIP:   remoteAddr,
			DstPort: dstPort,
		}
		instance, err := proxy.DialContext(ctx, metadata)
		if !assert.NoError(t, err) {
			return
		}
		defer instance.Close()

		transport := &http.Transport{
			DialContext: func(context.Context, string, string) (net.Conn, error) {
				return instance, nil
			},
			// from http.DefaultTransport
			MaxIdleConns:          100,
			IdleConnTimeout:       90 * time.Second,
			TLSHandshakeTimeout:   10 * time.Second,
			ExpectContinueTimeout: 1 * time.Second,
			// for our self-signed cert
			TLSClientConfig: tlsClientConfig.Clone(),
			// open http2
			ForceAttemptHTTP2: true,
		}

		client := http.Client{
			Timeout:   30 * time.Second,
			Transport: transport,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				return http.ErrUseLastResponse
			},
		}

		defer client.CloseIdleConnections()

		resp, err := client.Do(req)
		if !assert.NoError(t, err) {
			return
		}

		defer resp.Body.Close()

		assert.Equal(t, http.StatusOK, resp.StatusCode)

		data, err := io.ReadAll(resp.Body)
		if !assert.NoError(t, err) {
			return
		}
		assert.Equal(t, httpData, data)
	}
	tunnel := &TestTunnel{
		HandleTCPConnFn: func(conn net.Conn, metadata *C.Metadata) {
			defer conn.Close()
			if metadata.DstIP != remoteAddr && metadata.Host != realityDest {
				return // not match, just return
			}
			c := &WaitCloseConn{
				Conn: conn,
				ch:   make(chan struct{}),
			}
			if metadata.DstPort == 443 {
				tlsConn := tls.Server(c, tlsConfig.Clone())
				if metadata.Host == realityDest { // ignore the tls handshake error for realityDest
					ctx, cancel := context.WithTimeout(ctx, C.DefaultTLSTimeout)
					defer cancel()
					if err := tlsConn.HandshakeContext(ctx); err != nil {
						return
					}
				}
				ln.ch <- tlsConn
			} else {
				ln.ch <- c
			}
			<-c.ch
		},
		CloseFn: ln.Close,
		DoTestFn: func(t *testing.T, proxy C.ProxyAdapter) {
			// Sequential testing for debugging
			t.Run("Sequential", func(t *testing.T) {
				testFn(t, proxy, "http")
				testFn(t, proxy, "https")
			})

			// Concurrent testing to detect stress
			t.Run("Concurrent", func(t *testing.T) {
				wg := sync.WaitGroup{}
				const num = 50
				for i := 0; i < num; i++ {
					wg.Add(1)
					go func() {
						testFn(t, proxy, "https")
						defer wg.Done()
					}()
				}
				for i := 0; i < num; i++ {
					wg.Add(1)
					go func() {
						testFn(t, proxy, "http")
						defer wg.Done()
					}()
				}
				wg.Wait()
			})
		},
	}
	return tunnel
}
