package jls

import (
	"bytes"
	"context"
	"errors"
	"io"
	"net"
	"testing"
	"time"

	"github.com/metacubex/mihomo/component/ca"

	tls "github.com/metacubex/jls-tls"
)

func TestJLSClientServer(t *testing.T) {
	user := User{Username: "test-user", Password: "test-password"}
	serverConfig, err := NewServerConfig("camouflage.example", "camouflage.example:443", []User{user}, nil, 0, func(context.Context, string, string) (net.Conn, error) {
		return nil, errors.New("authenticated JLS connection dialed fallback")
	})
	if err != nil {
		t.Fatal(err)
	}
	clientConfig, err := NewClientConfig("camouflage.example", user.Username, user.Password, nil)
	if err != nil {
		t.Fatal(err)
	}

	serverSide, clientSide := net.Pipe()
	serverDone := make(chan error, 1)
	go func() {
		conn, err := Server(context.Background(), serverSide, serverConfig)
		if err != nil {
			serverDone <- err
			return
		}
		defer conn.Close()
		state := conn.(*Conn).ConnectionState()
		if !state.JLS.Authenticated || state.JLS.User != user.Username {
			serverDone <- errors.New("server did not authenticate JLS user")
			return
		}
		_, err = io.Copy(conn, conn)
		serverDone <- err
	}()

	client, err := NewClient(context.Background(), clientSide, clientConfig)
	if err != nil {
		t.Fatal(err)
	}
	payload := []byte("JLS over TCP")
	if _, err = client.Write(payload); err != nil {
		t.Fatal(err)
	}
	response := make([]byte, len(payload))
	if _, err = io.ReadFull(client, response); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(response, payload) {
		t.Fatalf("response = %q, want %q", response, payload)
	}
	_ = client.Close()
	if err = <-serverDone; err != nil {
		t.Fatal(err)
	}
}

func TestNewServerConfigRequiresDialContext(t *testing.T) {
	_, err := NewServerConfig(
		"camouflage.example",
		"camouflage.example:443",
		[]User{{Username: "user", Password: "password"}},
		nil,
		0,
		nil,
	)
	if err == nil || err.Error() != "jls: dial context is required" {
		t.Fatalf("error = %v, want dial context required error", err)
	}
}

func TestJLSServerFallback(t *testing.T) {
	for _, version := range []uint16{tls.VersionTLS13, tls.VersionTLS12} {
		t.Run(tls.VersionName(version), func(t *testing.T) {
			testJLSServerFallback(t, version)
		})
	}
}

func testJLSServerFallback(t *testing.T, version uint16) {
	upstreamConfig := newTestTLSServerConfig(t, version)
	upstreamClient, upstreamServer := net.Pipe()
	upstreamDone := make(chan error, 1)
	go func() {
		conn := tls.Server(upstreamServer, upstreamConfig)
		if err := conn.Handshake(); err != nil {
			upstreamDone <- err
			return
		}
		_, err := io.Copy(conn, conn)
		upstreamDone <- err
	}()

	serverConfig, err := NewServerConfig("camouflage.example", "camouflage.example:443", []User{{Username: "user", Password: "password"}}, nil, 0, func(context.Context, string, string) (net.Conn, error) {
		return upstreamClient, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	serverSide, clientSide := net.Pipe()
	serverDone := make(chan error, 1)
	go func() {
		_, err := Server(context.Background(), serverSide, serverConfig)
		serverDone <- err
	}()

	client := tls.Client(clientSide, &tls.Config{
		ServerName:         "camouflage.example",
		InsecureSkipVerify: true,
		MinVersion:         version,
		MaxVersion:         version,
	})
	if err = client.Handshake(); err != nil {
		t.Fatal(err)
	}
	payload := []byte("ordinary TLS fallback")
	if _, err = client.Write(payload); err != nil {
		t.Fatal(err)
	}
	response := make([]byte, len(payload))
	if _, err = io.ReadFull(client, response); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(response, payload) {
		t.Fatalf("response = %q, want %q", response, payload)
	}
	_ = client.Close()
	if err = <-serverDone; !errors.Is(err, ErrFallbackCompleted) {
		t.Fatalf("server error = %v, want %v", err, ErrFallbackCompleted)
	}
	if err = <-upstreamDone; err != nil {
		t.Fatal(err)
	}
}

func TestJLSServerDoesNotFallbackAfterAuthentication(t *testing.T) {
	user := User{Username: "user", Password: "password"}
	fallbackDialed := false
	serverConfig, err := NewServerConfig("camouflage.example", "camouflage.example:443", []User{user}, nil, 0, func(context.Context, string, string) (net.Conn, error) {
		fallbackDialed = true
		return nil, errors.New("fallback dialed")
	})
	if err != nil {
		t.Fatal(err)
	}
	serverConfig.TLSConfig.Certificates = nil

	clientConfig, err := NewClientConfig("camouflage.example", user.Username, user.Password, nil)
	if err != nil {
		t.Fatal(err)
	}
	serverSide, clientSide := net.Pipe()
	serverDone := make(chan error, 1)
	go func() {
		_, err := Server(context.Background(), serverSide, serverConfig)
		serverDone <- err
	}()

	client, clientErr := NewClient(context.Background(), clientSide, clientConfig)
	if client != nil {
		_ = client.Close()
	}
	_ = clientSide.Close()
	if clientErr == nil {
		t.Fatal("client handshake unexpectedly succeeded")
	}
	if serverErr := <-serverDone; serverErr == nil {
		t.Fatal("server handshake unexpectedly succeeded")
	}
	if fallbackDialed {
		t.Fatal("authenticated handshake failure dialed fallback")
	}
}

func TestJLSServerFallbackReplaysRejectedTLS(t *testing.T) {
	request := []byte{23, 3, 3, 0, 1, 0} // Application data before ClientHello.
	response := []byte("camouflage response")

	upstreamClient, upstreamServer := net.Pipe()
	upstreamDone := make(chan error, 1)
	go func() {
		defer upstreamServer.Close()
		got := make([]byte, len(request))
		if _, err := io.ReadFull(upstreamServer, got); err != nil {
			upstreamDone <- err
			return
		}
		if !bytes.Equal(got, request) {
			upstreamDone <- errors.New("fallback received modified handshake bytes")
			return
		}
		_, err := upstreamServer.Write(response)
		upstreamDone <- err
	}()

	serverConfig, err := NewServerConfig("camouflage.example", "camouflage.example:443", []User{{Username: "user", Password: "password"}}, nil, 0, func(context.Context, string, string) (net.Conn, error) {
		return upstreamClient, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	serverSide, clientSide := net.Pipe()
	serverDone := make(chan error, 1)
	go func() {
		_, err := Server(context.Background(), serverSide, serverConfig)
		serverDone <- err
	}()

	if _, err = clientSide.Write(request); err != nil {
		t.Fatal(err)
	}
	got := make([]byte, len(response))
	if _, err = io.ReadFull(clientSide, got); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, response) {
		t.Fatalf("fallback response = %q, want %q", got, response)
	}
	_ = clientSide.Close()
	if err = <-serverDone; !errors.Is(err, ErrFallbackCompleted) {
		t.Fatalf("server error = %v, want %v", err, ErrFallbackCompleted)
	}
	if err = <-upstreamDone; err != nil {
		t.Fatal(err)
	}
}

func TestBitRateLimiterReservations(t *testing.T) {
	limiter := &bitRateLimiter{rateBps: 800}
	now := time.Unix(0, 0)
	if delay := limiter.reserveN(now, 1); delay != 0 {
		t.Fatalf("initial reservation delay = %s, want 0", delay)
	}
	if delay := limiter.reserveN(now, 1); delay != 10*time.Millisecond {
		t.Fatalf("second reservation delay = %s, want 10ms", delay)
	}
}

func newTestTLSServerConfig(t *testing.T, version uint16) *tls.Config {
	t.Helper()
	certificatePEM, privateKeyPEM, _, err := ca.NewRandomTLSKeyPair(ca.KeyPairTypeP256)
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := tls.X509KeyPair([]byte(certificatePEM), []byte(privateKeyPEM))
	if err != nil {
		t.Fatal(err)
	}
	return &tls.Config{
		Certificates: []tls.Certificate{certificate},
		MinVersion:   version,
		MaxVersion:   version,
	}
}
