//go:build darwin && cgo

package tls

import (
	"context"
	stdtls "crypto/tls"
	"net"
	"testing"
	"time"

	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common/json/badoption"
	"github.com/sagernet/sing/common/logger"
)

const appleTLSTestTimeout = 5 * time.Second

const (
	appleTLSSuccessHandshakeLoops = 20
	appleTLSFailureRecoveryLoops  = 10
)

type appleTLSServerResult struct {
	state stdtls.ConnectionState
	err   error
}

func TestAppleClientHandshakeAppliesALPNAndVersion(t *testing.T) {
	serverCertificate, serverCertificatePEM := newAppleTestCertificate(t, "localhost")
	for index := 0; index < appleTLSSuccessHandshakeLoops; index++ {
		serverResult, serverAddress := startAppleTLSTestServer(t, &stdtls.Config{
			Certificates: []stdtls.Certificate{serverCertificate},
			MinVersion:   stdtls.VersionTLS12,
			MaxVersion:   stdtls.VersionTLS12,
			NextProtos:   []string{"h2"},
		})

		clientConn, err := newAppleTestClientConn(t, serverAddress, option.OutboundTLSOptions{
			Enabled:     true,
			Engine:      "apple",
			ServerName:  "localhost",
			MinVersion:  "1.2",
			MaxVersion:  "1.2",
			ALPN:        badoption.Listable[string]{"h2"},
			Certificate: badoption.Listable[string]{serverCertificatePEM},
		})
		if err != nil {
			t.Fatalf("iteration %d: %v", index, err)
		}

		clientState := clientConn.ConnectionState()
		if clientState.Version != stdtls.VersionTLS12 {
			_ = clientConn.Close()
			t.Fatalf("iteration %d: unexpected negotiated version: %x", index, clientState.Version)
		}
		if clientState.NegotiatedProtocol != "h2" {
			_ = clientConn.Close()
			t.Fatalf("iteration %d: unexpected negotiated protocol: %q", index, clientState.NegotiatedProtocol)
		}
		_ = clientConn.Close()

		result := <-serverResult
		if result.err != nil {
			t.Fatalf("iteration %d: %v", index, result.err)
		}
		if result.state.Version != stdtls.VersionTLS12 {
			t.Fatalf("iteration %d: server negotiated unexpected version: %x", index, result.state.Version)
		}
		if result.state.NegotiatedProtocol != "h2" {
			t.Fatalf("iteration %d: server negotiated unexpected protocol: %q", index, result.state.NegotiatedProtocol)
		}
	}
}

func TestAppleClientHandshakeRejectsVersionMismatch(t *testing.T) {
	serverCertificate, serverCertificatePEM := newAppleTestCertificate(t, "localhost")
	serverResult, serverAddress := startAppleTLSTestServer(t, &stdtls.Config{
		Certificates: []stdtls.Certificate{serverCertificate},
		MinVersion:   stdtls.VersionTLS13,
		MaxVersion:   stdtls.VersionTLS13,
	})

	clientConn, err := newAppleTestClientConn(t, serverAddress, option.OutboundTLSOptions{
		Enabled:     true,
		Engine:      "apple",
		ServerName:  "localhost",
		MaxVersion:  "1.2",
		Certificate: badoption.Listable[string]{serverCertificatePEM},
	})
	if err == nil {
		clientConn.Close()
		t.Fatal("expected version mismatch handshake to fail")
	}

	if result := <-serverResult; result.err == nil {
		t.Fatal("expected server handshake to fail on version mismatch")
	}
}

func TestAppleClientHandshakeRejectsServerNameMismatch(t *testing.T) {
	serverCertificate, serverCertificatePEM := newAppleTestCertificate(t, "localhost")
	serverResult, serverAddress := startAppleTLSTestServer(t, &stdtls.Config{
		Certificates: []stdtls.Certificate{serverCertificate},
	})

	clientConn, err := newAppleTestClientConn(t, serverAddress, option.OutboundTLSOptions{
		Enabled:     true,
		Engine:      "apple",
		ServerName:  "example.com",
		Certificate: badoption.Listable[string]{serverCertificatePEM},
	})
	if err == nil {
		clientConn.Close()
		t.Fatal("expected server name mismatch handshake to fail")
	}

	if result := <-serverResult; result.err == nil {
		t.Fatal("expected server handshake to fail on server name mismatch")
	}
}

func TestAppleClientHandshakeRecoversAfterFailure(t *testing.T) {
	serverCertificate, serverCertificatePEM := newAppleTestCertificate(t, "localhost")
	testCases := []struct {
		name          string
		serverConfig  *stdtls.Config
		clientOptions option.OutboundTLSOptions
	}{
		{
			name: "version mismatch",
			serverConfig: &stdtls.Config{
				Certificates: []stdtls.Certificate{serverCertificate},
				MinVersion:   stdtls.VersionTLS13,
				MaxVersion:   stdtls.VersionTLS13,
			},
			clientOptions: option.OutboundTLSOptions{
				Enabled:     true,
				Engine:      "apple",
				ServerName:  "localhost",
				MaxVersion:  "1.2",
				Certificate: badoption.Listable[string]{serverCertificatePEM},
			},
		},
		{
			name: "server name mismatch",
			serverConfig: &stdtls.Config{
				Certificates: []stdtls.Certificate{serverCertificate},
			},
			clientOptions: option.OutboundTLSOptions{
				Enabled:     true,
				Engine:      "apple",
				ServerName:  "example.com",
				Certificate: badoption.Listable[string]{serverCertificatePEM},
			},
		},
	}
	successClientOptions := option.OutboundTLSOptions{
		Enabled:     true,
		Engine:      "apple",
		ServerName:  "localhost",
		MinVersion:  "1.2",
		MaxVersion:  "1.2",
		ALPN:        badoption.Listable[string]{"h2"},
		Certificate: badoption.Listable[string]{serverCertificatePEM},
	}

	for _, testCase := range testCases {
		t.Run(testCase.name, func(t *testing.T) {
			for index := 0; index < appleTLSFailureRecoveryLoops; index++ {
				failedResult, failedAddress := startAppleTLSTestServer(t, testCase.serverConfig)
				failedConn, err := newAppleTestClientConn(t, failedAddress, testCase.clientOptions)
				if err == nil {
					_ = failedConn.Close()
					t.Fatalf("iteration %d: expected handshake failure", index)
				}
				if result := <-failedResult; result.err == nil {
					t.Fatalf("iteration %d: expected server handshake failure", index)
				}

				successResult, successAddress := startAppleTLSTestServer(t, &stdtls.Config{
					Certificates: []stdtls.Certificate{serverCertificate},
					MinVersion:   stdtls.VersionTLS12,
					MaxVersion:   stdtls.VersionTLS12,
					NextProtos:   []string{"h2"},
				})
				successConn, err := newAppleTestClientConn(t, successAddress, successClientOptions)
				if err != nil {
					t.Fatalf("iteration %d: follow-up handshake failed: %v", index, err)
				}
				clientState := successConn.ConnectionState()
				if clientState.NegotiatedProtocol != "h2" {
					_ = successConn.Close()
					t.Fatalf("iteration %d: unexpected negotiated protocol after failure: %q", index, clientState.NegotiatedProtocol)
				}
				_ = successConn.Close()

				result := <-successResult
				if result.err != nil {
					t.Fatalf("iteration %d: follow-up server handshake failed: %v", index, result.err)
				}
				if result.state.NegotiatedProtocol != "h2" {
					t.Fatalf("iteration %d: follow-up server negotiated unexpected protocol: %q", index, result.state.NegotiatedProtocol)
				}
			}
		})
	}
}

func newAppleTestCertificate(t *testing.T, serverName string) (stdtls.Certificate, string) {
	t.Helper()

	privateKeyPEM, certificatePEM, err := GenerateCertificate(nil, nil, time.Now, serverName, time.Now().Add(time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := stdtls.X509KeyPair(certificatePEM, privateKeyPEM)
	if err != nil {
		t.Fatal(err)
	}
	return certificate, string(certificatePEM)
}

func startAppleTLSTestServer(t *testing.T, tlsConfig *stdtls.Config) (<-chan appleTLSServerResult, string) {
	t.Helper()

	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		listener.Close()
	})

	if tcpListener, isTCP := listener.(*net.TCPListener); isTCP {
		err = tcpListener.SetDeadline(time.Now().Add(appleTLSTestTimeout))
		if err != nil {
			t.Fatal(err)
		}
	}

	result := make(chan appleTLSServerResult, 1)
	go func() {
		defer close(result)

		conn, err := listener.Accept()
		if err != nil {
			result <- appleTLSServerResult{err: err}
			return
		}
		defer conn.Close()

		err = conn.SetDeadline(time.Now().Add(appleTLSTestTimeout))
		if err != nil {
			result <- appleTLSServerResult{err: err}
			return
		}

		tlsConn := stdtls.Server(conn, tlsConfig)
		defer tlsConn.Close()

		err = tlsConn.Handshake()
		if err != nil {
			result <- appleTLSServerResult{err: err}
			return
		}

		result <- appleTLSServerResult{state: tlsConn.ConnectionState()}
	}()

	return result, listener.Addr().String()
}

func newAppleTestClientConn(t *testing.T, serverAddress string, options option.OutboundTLSOptions) (Conn, error) {
	t.Helper()

	ctx, cancel := context.WithTimeout(context.Background(), appleTLSTestTimeout)
	t.Cleanup(cancel)

	clientConfig, err := NewClientWithOptions(ClientOptions{
		Context:       ctx,
		Logger:        logger.NOP(),
		ServerAddress: "",
		Options:       options,
	})
	if err != nil {
		return nil, err
	}

	conn, err := net.DialTimeout("tcp", serverAddress, appleTLSTestTimeout)
	if err != nil {
		return nil, err
	}

	tlsConn, err := ClientHandshake(ctx, conn, clientConfig)
	if err != nil {
		conn.Close()
		return nil, err
	}
	return tlsConn, nil
}
