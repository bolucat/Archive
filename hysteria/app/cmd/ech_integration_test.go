package cmd

import (
	"crypto/tls"
	"encoding/base64"
	"errors"
	"io"
	"net"
	"path/filepath"
	"sync/atomic"
	"testing"
	"time"

	"github.com/apernet/hysteria/app/v2/internal/utils"
	"github.com/apernet/hysteria/core/v2/client"
	"github.com/apernet/hysteria/core/v2/server"
	"github.com/stretchr/testify/require"
	"go.uber.org/zap"
)

// Exercise CLI generation, the existing file/config loaders, certificate
// verification, ECH acceptance, and Hysteria's TCP/UDP proxy protocols together.
func TestECHCommandHysteriaIntegration(t *testing.T) {
	previousLogger := logger
	logger = zap.NewNop()
	t.Cleanup(func() { logger = previousLogger })
	dir := t.TempDir()
	certFile, keyFile := filepath.Join(dir, "server.crt"), filepath.Join(dir, "server.key")
	_, err := runCert(certOptions{
		Hosts: "inner.example.com,public.example.com", CertFile: certFile, KeyFile: keyFile, ValidFor: time.Hour,
	})
	require.NoError(t, err)
	for _, aead := range []string{"aes-128-gcm", "aes-256-gcm", "chacha20-poly1305"} {
		t.Run(aead, func(t *testing.T) {
			path := filepath.Join(dir, aead+".pem")
			_, err := executeECH("--public-name", "public.example.com", "--output", path,
				"--config-id", "42", "--max-name-length", "64", "--aead", aead)
			require.NoError(t, err)
			_, list, err := utils.LoadECHKeys(path)
			require.NoError(t, err)
			serverConfig := serverConfig{
				TLS:  &serverConfigTLS{Cert: certFile, Key: keyFile},
				ECH:  &serverConfigECH{KeyPath: path},
				Auth: serverConfigAuth{Type: "password", Password: "test-password"},
			}
			packetConn, err := net.ListenPacket("udp", "127.0.0.1:0")
			require.NoError(t, err)
			t.Cleanup(func() { _ = packetConn.Close() })
			sc := &server.Config{Conn: packetConn}
			require.NoError(t, serverConfig.fillTLSConfig(sc))
			require.NoError(t, serverConfig.fillAuthenticator(sc))
			var outerName atomic.Value
			outerName.Store("")
			sc.TLSConfig.GetECHKeys = func(info *tls.ClientHelloInfo) ([]tls.EncryptedClientHelloKey, error) {
				// This callback also runs after decryption to build retry configs.
				// Record only its first invocation, which sees the outer ClientHello.
				outerName.CompareAndSwap("", info.ServerName)
				return sc.TLSConfig.ECHKeys, nil
			}
			s, err := server.NewServer(sc)
			require.NoError(t, err)
			t.Cleanup(func() { _ = s.Close() })
			go s.Serve()

			for _, disableParrot := range []bool{false, true} {
				// Cover inline base64, PEM file input, and normal clients without ECH.
				for _, input := range []string{base64.StdEncoding.EncodeToString(list), path, ""} {
					cc := &client.Config{ServerAddr: packetConn.LocalAddr(), Auth: "test-password", QUICConfig: client.QUICConfig{DisableChromeParrot: disableParrot}}
					config := clientConfig{TLS: clientConfigTLS{SNI: "inner.example.com", CA: certFile, ECH: input}}
					require.NoError(t, config.fillTLSConfig(cc))
					outerName.Store("")
					c, info, err := client.NewClient(cc)
					require.NoError(t, err)
					t.Cleanup(func() { _ = c.Close() })
					if input != "" {
						require.Equal(t, "public.example.com", outerName.Load(), "outer SNI must not expose the inner name")
					}
					// Chrome parroting currently omits ECHAccepted from its state copy;
					// outer SNI and fail-closed checks below verify ECH on both paths.
					if disableParrot {
						require.Equal(t, input != "", info.ECHAccepted)
					}
					assertECHProxyTraffic(t, c)
					require.NoError(t, c.Close())
				}

				// A fresh, unrelated key must fail closed rather than silently using
				// cleartext SNI. Trust both names so this tests ECH rejection itself.
				_, wrongList, err := utils.GenerateECHKeys(utils.ECHKeyOptions{PublicName: "public.example.com"})
				require.NoError(t, err)
				cc := &client.Config{ServerAddr: packetConn.LocalAddr(), Auth: "test-password", QUICConfig: client.QUICConfig{DisableChromeParrot: disableParrot}}
				config := clientConfig{TLS: clientConfigTLS{
					SNI: "inner.example.com", CA: certFile, ECH: base64.StdEncoding.EncodeToString(wrongList),
				}}
				require.NoError(t, config.fillTLSConfig(cc))
				c, _, err := client.NewClient(cc)
				if c != nil {
					_ = c.Close()
				}
				require.Error(t, err)
				require.ErrorContains(t, err, "server rejected ECH")
				if disableParrot {
					var rejected *tls.ECHRejectionError
					require.True(t, errors.As(err, &rejected), "expected ECH rejection, got %v", err)
					require.Equal(t, list, rejected.RetryConfigList)
				}
			}
		})
	}
}

func assertECHProxyTraffic(t *testing.T, c client.Client) {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	defer listener.Close()
	go func() {
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		defer conn.Close()
		_ = conn.SetDeadline(time.Now().Add(5 * time.Second))
		_, _ = io.Copy(conn, conn)
	}()
	conn, err := c.TCP(listener.Addr().String())
	require.NoError(t, err)
	defer conn.Close()
	require.NoError(t, conn.SetDeadline(time.Now().Add(5*time.Second)))
	_, err = conn.Write([]byte("ECH TCP"))
	require.NoError(t, err)
	buf := make([]byte, 7)
	_, err = io.ReadFull(conn, buf)
	require.NoError(t, err)
	require.Equal(t, "ECH TCP", string(buf))

	udp, err := net.ListenPacket("udp", "127.0.0.1:0")
	require.NoError(t, err)
	defer udp.Close()
	go func() {
		_ = udp.SetDeadline(time.Now().Add(5 * time.Second))
		buf := make([]byte, 1024)
		n, addr, err := udp.ReadFrom(buf)
		if err == nil {
			_, _ = udp.WriteTo(buf[:n], addr)
		}
	}()
	u, err := c.UDP()
	require.NoError(t, err)
	defer u.Close()
	require.NoError(t, u.Send([]byte("ECH UDP"), udp.LocalAddr().String()))
	type result struct {
		data []byte
		err  error
	}
	ch := make(chan result, 1)
	go func() {
		data, _, err := u.Receive()
		ch <- result{data, err}
	}()
	select {
	case r := <-ch:
		require.NoError(t, r.err)
		require.Equal(t, "ECH UDP", string(r.data))
	case <-time.After(5 * time.Second):
		t.Fatal("timed out waiting for proxied UDP response")
	}
}
