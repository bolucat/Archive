package integration_tests

import (
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"math/rand"
	"net"
	"os"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/apernet/quic-go"
	"github.com/stretchr/testify/mock"
	"github.com/stretchr/testify/require"
	"go.uber.org/goleak"
	"golang.org/x/time/rate"

	"github.com/apernet/hysteria/core/v2/client"
	"github.com/apernet/hysteria/core/v2/internal/integration_tests/mocks"
	"github.com/apernet/hysteria/core/v2/server"
)

const stressBufferSize = 32 * 1024

// Every case owns its transport: canceling it must interrupt TCP setup and
// QUIC DATAGRAM sends, which have no per-operation context/deadline API.
func newStressClient(t *testing.T, timeout time.Duration) (client.Client, context.Context, context.CancelCauseFunc) {
	t.Helper()
	leakCheck := goleak.IgnoreCurrent()
	t.Cleanup(func() { goleak.VerifyNone(t, leakCheck) })
	deadline := time.Now().Add(timeout)
	if d, ok := t.Deadline(); ok && d.Add(-5*time.Second).Before(deadline) {
		deadline = d.Add(-5 * time.Second)
	}
	timedCtx, stopTimer := context.WithDeadline(t.Context(), deadline)
	ctx, cancel := context.WithCancelCause(timedCtx)
	t.Cleanup(stopTimer)
	t.Cleanup(func() { cancel(nil) })

	pc, err := net.ListenPacket("udp", "127.0.0.1:0")
	require.NoError(t, err)
	t.Cleanup(func() { _ = pc.Close() })
	auth := mocks.NewMockAuthenticator(t)
	auth.EXPECT().Authenticate(mock.Anything, mock.Anything, mock.Anything).Return(true, "stress")
	s, err := server.NewServer(&server.Config{
		TLSConfig: serverTLSConfig(), Conn: pc, Authenticator: auth,
	})
	require.NoError(t, err)
	served := make(chan error, 1)
	go func() { served <- s.Serve() }()
	t.Cleanup(func() {
		_ = s.Close()
		<-served
	})

	// Own the socket before NewClient so the case deadline also bounds auth.
	clientPC, err := net.ListenPacket("udp", "127.0.0.1:0")
	require.NoError(t, err)
	t.Cleanup(stressCloseOnCancel(ctx, func() { _ = clientPC.Close() }))
	c, _, err := client.NewClient(&client.Config{
		ConnFactory: stressConnFactory{clientPC},
		ServerAddr:  pc.LocalAddr(), TLSConfig: client.TLSConfig{InsecureSkipVerify: true},
	})
	require.NoError(t, err)
	t.Cleanup(stressCloseOnCancel(ctx, func() { _ = c.Close() }))
	return c, ctx, cancel
}

type stressConnFactory struct{ net.PacketConn }

func (f stressConnFactory) New(net.Addr) (net.PacketConn, error) { return f.PacketConn, nil }

// Stop and join the cancellation callback as well as closing the resource.
func stressCloseOnCancel(ctx context.Context, closeFn func()) func() {
	closeOnce := sync.OnceFunc(closeFn)
	done := make(chan struct{})
	stop := context.AfterFunc(ctx, func() {
		defer close(done)
		closeOnce()
	})
	return func() {
		if !stop() {
			<-done
		}
		closeOnce()
	}
}

func runStressWorkers(ctx context.Context, cancel context.CancelCauseFunc, iterations, parallel int, work func(context.Context, int) error) error {
	for iteration := 0; iteration < iterations; iteration++ {
		var wg sync.WaitGroup
		failures := make(chan error, 1)
		for worker := 0; worker < parallel; worker++ {
			id := iteration*parallel + worker
			wg.Go(func() {
				if ctx.Err() != nil {
					return
				}
				if err := work(ctx, id); err != nil {
					err = fmt.Errorf("iteration %d worker %d: %w", iteration, worker, err)
					select {
					case failures <- err:
					default:
					}
					cancel(err)
				}
			})
		}
		wg.Wait()
		if ctx.Err() != nil {
			cause := context.Cause(ctx)
			select {
			case err := <-failures:
				// Keep operation progress even when the case deadline fired first.
				if errors.Is(err, cause) {
					return err
				}
				return errors.Join(cause, err)
			default:
				return cause
			}
		}
	}
	return nil
}

func dialStressTCP(ctx context.Context, c client.Client, addr string) (net.Conn, error) {
	for {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		conn, err := c.TCP(addr)
		if !errors.Is(err, quic.StreamLimitReachedError{}) {
			return conn, err
		}
		// Stream credit is replenished asynchronously after earlier streams close.
		timer := time.NewTimer(10 * time.Millisecond)
		select {
		case <-ctx.Done():
			timer.Stop()
			return nil, ctx.Err()
		case <-timer.C:
		}
	}
}

// Generate and check a reproducible, distinct byte stream for each connection.
// Memory is O(parallel * buffer size), independent of the transfer size.
func stressTCPTransfer(ctx context.Context, conn net.Conn, size, seed int64) error {
	defer stressCloseOnCancel(ctx, func() { _ = conn.Close() })()
	if deadline, ok := ctx.Deadline(); ok {
		if err := conn.SetDeadline(deadline); err != nil {
			return err
		}
	}
	written := make(chan error, 1)
	go func() {
		_, err := io.CopyBuffer(conn, io.LimitReader(rand.New(rand.NewSource(seed)), size), make([]byte, stressBufferSize))
		if err != nil {
			_ = conn.Close()
		}
		written <- err
	}()
	expected := rand.New(rand.NewSource(seed))
	want, got := make([]byte, stressBufferSize), make([]byte, stressBufferSize)
	var readErr error
	for offset := int64(0); offset < size; {
		n := int(min(int64(len(got)), size-offset))
		if _, err := io.ReadFull(conn, got[:n]); err != nil {
			readErr = fmt.Errorf("read at byte %d/%d: %w", offset, size, err)
			break
		}
		_, _ = expected.Read(want[:n])
		if !bytes.Equal(got[:n], want[:n]) {
			readErr = fmt.Errorf("payload mismatch at byte %d/%d", offset, size)
			break
		}
		offset += int64(n)
	}
	if readErr != nil {
		_ = conn.Close()
	}
	writeErr := <-written
	if writeErr != nil {
		writeErr = fmt.Errorf("write: %w", writeErr)
	}
	return errors.Join(readErr, writeErr)
}

type stressUDPPacket struct {
	data []byte
	addr string
	err  error
}

type stressUDPStats struct{ sent, received, duplicates int }

// Payload identity catches corruption, duplicate counting, and cross-session
// delivery. The packet number permits loss and reordering without retries.
func stressUDPPayload(size, session, sequence int) []byte {
	data := make([]byte, size)
	_, _ = rand.New(rand.NewSource(int64(session))).Read(data)
	binary.BigEndian.PutUint64(data, uint64(sequence))
	return data
}

func stressUDPTransfer(ctx context.Context, abort context.CancelCauseFunc, conn client.HyUDPConn, addr string, size, count, session int, drain time.Duration, pace func(context.Context) error) (stats stressUDPStats, result error) {
	localCtx, cancel := context.WithCancel(ctx)
	received := make(chan stressUDPPacket, 1)
	sent := make(chan error, 1)
	var sendCount atomic.Int64
	var wg sync.WaitGroup
	wg.Go(func() {
		for {
			data, addr, err := conn.Receive()
			select {
			case received <- stressUDPPacket{data, addr, err}:
			case <-localCtx.Done():
				return
			}
			if err != nil {
				return
			}
		}
	})
	wg.Go(func() {
		for sequence := 0; sequence < count; sequence++ {
			if err := pace(localCtx); err != nil {
				sent <- err
				return
			}
			if err := conn.Send(stressUDPPayload(size, session, sequence), addr); err != nil {
				sent <- fmt.Errorf("send packet %d: %w", sequence, err)
				return
			}
			sendCount.Add(1)
		}
		sent <- nil
	})
	defer func() {
		// A failed worker cancels the fixture, closing the QUIC transport to
		// unblock a Send stuck in its queue. Closing a UDP session only wakes Receive.
		if result != nil {
			abort(result)
		}
		cancel()
		_ = conn.Close()
		wg.Wait()
		stats.sent = int(sendCount.Load())
	}()
	seen := make([]bool, count)
	var drained <-chan time.Time
	timer := time.NewTimer(time.Hour)
	timer.Stop()
	defer timer.Stop()
	for {
		select {
		case <-ctx.Done():
			return stats, fmt.Errorf("sent/received %d/%d of %d: %w", sendCount.Load(), stats.received, count, context.Cause(ctx))
		case err := <-sent:
			if err != nil {
				return stats, err
			}
			stats.sent = count
			sent = nil
			timer.Reset(drain)
			drained = timer.C
		case packet := <-received:
			if packet.err != nil {
				return stats, fmt.Errorf("receive: %w", packet.err)
			}
			if packet.addr != addr {
				return stats, fmt.Errorf("source address %q, want %q", packet.addr, addr)
			}
			if len(packet.data) != size {
				return stats, fmt.Errorf("payload size %d, want %d", len(packet.data), size)
			}
			sequence := binary.BigEndian.Uint64(packet.data)
			if sequence >= uint64(count) {
				return stats, fmt.Errorf("invalid packet number %d", sequence)
			}
			if !bytes.Equal(packet.data, stressUDPPayload(size, session, int(sequence))) {
				return stats, fmt.Errorf("payload mismatch in packet %d", sequence)
			}
			if seen[sequence] {
				stats.duplicates++
			} else {
				seen[sequence] = true
				stats.received++
			}
		case <-drained:
			// Ceiling matters for small counts: always require at least one reply.
			minimum := (count*8 + 9) / 10
			if stats.received < minimum {
				return stats, fmt.Errorf("UDP loss: received %d/%d unique packets (minimum %d, duplicates %d)", stats.received, count, minimum, stats.duplicates)
			}
			return stats, nil
		}
		if stats.sent == count && stats.received == count {
			return stats, nil
		}
	}
}

func stressTCPEcho(t *testing.T) string {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	var wg sync.WaitGroup
	var mu sync.Mutex
	conns := make(map[net.Conn]struct{})
	done := make(chan struct{})
	go func() {
		defer close(done)
		for {
			conn, err := listener.Accept()
			if err != nil {
				return
			}
			mu.Lock()
			conns[conn] = struct{}{}
			mu.Unlock()
			wg.Go(func() {
				defer conn.Close()
				_, _ = io.Copy(conn, conn)
				mu.Lock()
				delete(conns, conn)
				mu.Unlock()
			})
		}
	}()
	t.Cleanup(func() {
		_ = listener.Close()
		<-done // No more accepts or WaitGroup additions.
		mu.Lock()
		for conn := range conns {
			_ = conn.Close()
		}
		mu.Unlock()
		wg.Wait()
	})
	return listener.Addr().String()
}

func stressUDPEcho(t *testing.T) string {
	t.Helper()
	conn, err := net.ListenPacket("udp6", "[::1]:0")
	if err != nil {
		conn, err = net.ListenPacket("udp4", "127.0.0.1:0")
	}
	require.NoError(t, err)
	s := &udpEchoServer{Conn: conn}
	done := make(chan error, 1)
	go func() { done <- s.Serve() }()
	t.Cleanup(func() { _ = s.Close(); <-done })
	return conn.LocalAddr().String()
}

func TestClientServerTCPStress(t *testing.T) {
	cases := []struct {
		name                 string
		size                 int64
		parallel, iterations int
	}{
		{"single", 32 << 20, 1, 1},
		{"sequential", 64 << 10, 1, 128},
		{"small_stream_churn", 4 << 10, 1, 256},
		{"parallel", 1 << 20, 32, 1},
		{"parallel_stream_churn", 64 << 10, 128, 4},
	}
	timeout := 30 * time.Second
	if os.Getenv("HYSTERIA_STRESS_EXTENDED") == "1" && !testing.Short() {
		cases = append(cases, []struct {
			name                 string
			size                 int64
			parallel, iterations int
		}{
			{"extended_single_500MiB", 500 << 20, 1, 1},
			{"extended_sequential_1000x1MiB", 1 << 20, 1, 1000},
			{"extended_sequential_10000x100KiB", 100 << 10, 1, 10000},
			{"extended_parallel_100x10MiB", 10 << 20, 100, 1},
			{"extended_parallel_1000x1MiB", 1 << 20, 1000, 1},
		}...)
		timeout = 2 * time.Minute
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			c, ctx, cancel := newStressClient(t, timeout)
			addr := stressTCPEcho(t)
			started := time.Now()
			err := runStressWorkers(ctx, cancel, tc.iterations, tc.parallel, func(ctx context.Context, id int) error {
				conn, err := dialStressTCP(ctx, c, addr)
				if err != nil {
					return fmt.Errorf("dial: %w", err)
				}
				return stressTCPTransfer(ctx, conn, tc.size, int64(id))
			})
			require.NoError(t, err)
			t.Logf("verified %d streams, %d bytes each, parallel=%d in %s", tc.iterations*tc.parallel, tc.size, tc.parallel, time.Since(started))
		})
	}
}

func TestClientServerUDPStress(t *testing.T) {
	cases := []struct {
		name                              string
		size, count, parallel, iterations int
	}{
		{"small", 100, 200, 1, 1},
		{"fragmented", 3000, 200, 1, 1},
		{"small_session_churn", 100, 100, 8, 3},
		{"fragmented_session_churn", 3000, 100, 8, 3},
	}
	timeout := 30 * time.Second
	if os.Getenv("HYSTERIA_STRESS_EXTENDED") == "1" && !testing.Short() {
		cases = append(cases, []struct {
			name                              string
			size, count, parallel, iterations int
		}{
			{"extended_small", 100, 1000, 1, 1},
			{"extended_fragmented", 3000, 1000, 1, 1},
			{"extended_small_sequential", 100, 1000, 1, 5},
			{"extended_fragmented_sequential", 3000, 200, 1, 5},
			{"extended_small_parallel", 100, 1000, 5, 2},
			{"extended_fragmented_parallel", 3000, 200, 5, 2},
		}...)
		timeout = 2 * time.Minute
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			c, ctx, cancel := newStressClient(t, timeout)
			addr := stressUDPEcho(t)
			// A byte limit alone permits enormous bursts of small packets. Bound
			// both packet rate and byte rate globally, with only one packet of burst.
			packets := rate.NewLimiter(1000, 1)
			bandwidth := rate.NewLimiter(1<<20, tc.size)
			pace := func(ctx context.Context) error {
				if err := packets.Wait(ctx); err != nil {
					return err
				}
				return bandwidth.WaitN(ctx, tc.size)
			}
			var mu sync.Mutex
			var total stressUDPStats
			err := runStressWorkers(ctx, cancel, tc.iterations, tc.parallel, func(ctx context.Context, id int) error {
				conn, err := c.UDP()
				if err != nil {
					return err
				}
				abort := func(err error) { cancel(fmt.Errorf("session %d: %w", id, err)) }
				stats, err := stressUDPTransfer(ctx, abort, conn, addr, tc.size, tc.count, id, time.Second, pace)
				mu.Lock()
				total.sent += stats.sent
				total.received += stats.received
				total.duplicates += stats.duplicates
				mu.Unlock()
				return err
			})
			t.Logf("sent=%d unique replies=%d duplicates=%d", total.sent, total.received, total.duplicates)
			require.NoError(t, err)
		})
	}
}
