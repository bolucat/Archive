package mekya

import (
	"bytes"
	"context"
	"io"
	"net"
	"testing"
	"time"

	"github.com/metacubex/mihomo/transport/mkcp"

	"github.com/stretchr/testify/require"
)

func TestRoundTrip(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	cfg := testConfig()
	server, err := Listen(ctx, ln, cfg)
	require.NoError(t, err)
	defer server.Close()

	serverErr := make(chan error, 1)
	go func() {
		conn, err := server.Accept()
		if err != nil {
			serverErr <- err
			return
		}
		defer conn.Close()
		serverErr <- echo(conn)
	}()

	client, err := NewClient(ctx, func(ctx context.Context) (net.Conn, error) {
		var d net.Dialer
		return d.DialContext(ctx, "tcp", server.Addr().String())
	}, cfg)
	require.NoError(t, err)
	defer client.Close()

	conn, err := client.Dial(ctx)
	require.NoError(t, err)
	defer conn.Close()
	require.NoError(t, conn.SetDeadline(time.Now().Add(5*time.Second)))

	payload := bytes.Repeat([]byte("m"), 64*1024)
	_, err = conn.Write(payload)
	require.NoError(t, err)
	got := make([]byte, len(payload))
	_, err = io.ReadFull(conn, got)
	require.NoError(t, err)
	require.Equal(t, payload, got)
	require.NotNil(t, conn.LocalAddr())
	require.NotNil(t, conn.RemoteAddr())
	require.NoError(t, conn.Close())
}

func testConfig() Config {
	return Config{
		KCP: mkcp.Config{
			TTI: 15,
		},
		URL:                            "https://example.invalid/mekya",
		H2PoolSize:                     2,
		MaxWriteDelay:                  20,
		MaxRequestSize:                 96000,
		PollingIntervalInitial:         20,
		MaxWriteSize:                   1 << 20,
		MaxWriteDurationMs:             100,
		MaxSimultaneousWriteConnection: 16,
		PacketWritingBuffer:            1024,
	}
}

func echo(conn net.Conn) error {
	_, err := io.Copy(conn, conn)
	return err
}
