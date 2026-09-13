package integration_tests

import (
	"context"
	"encoding/binary"
	"errors"
	"io"
	"net"
	"sync"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"go.uber.org/goleak"
)

func TestStressTCPTransfer(t *testing.T) {
	for _, behavior := range []string{"echo", "corrupt", "early_close", "write_error", "blocked_read_and_write"} {
		t.Run(behavior, func(t *testing.T) {
			defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
			ctx, cancel := context.WithTimeout(t.Context(), time.Second)
			defer cancel()
			a, b := net.Pipe()
			defer b.Close()
			started, release := make(chan struct{}), make(chan struct{})
			conn := &stressWriteSignalConn{Conn: a, signal: sync.OnceFunc(func() { close(started) })}
			writeFailure := errors.New("injected write failure")
			if behavior == "write_error" {
				conn.writeErr = writeFailure
			}
			done := make(chan struct{})
			go func() {
				defer close(done)
				defer b.Close()
				switch behavior {
				case "echo":
					_, _ = io.Copy(b, b)
				case "corrupt":
					buf := make([]byte, stressBufferSize)
					n, _ := b.Read(buf)
					buf[0] ^= 0xff
					_, _ = b.Write(buf[:n])
				case "blocked_read_and_write":
					// Leave the peer open and idle until the transfer has returned.
					// Only cancellation can release the blocked Read and Write.
					<-started
					cancel()
					<-release
				case "write_error":
					<-release
				}
			}()
			err := stressTCPTransfer(ctx, conn, 2*stressBufferSize+7, 42)
			close(release)
			<-done
			if behavior == "echo" {
				require.NoError(t, err)
			} else {
				require.Error(t, err)
				if behavior == "corrupt" {
					require.ErrorContains(t, err, "payload mismatch")
				}
				if behavior == "write_error" {
					require.ErrorIs(t, err, writeFailure)
				}
			}
		})
	}
}

type stressWriteSignalConn struct {
	net.Conn
	signal   func()
	writeErr error
}

func (c *stressWriteSignalConn) Write(data []byte) (int, error) {
	c.signal()
	if c.writeErr != nil {
		return 0, c.writeErr
	}
	return c.Conn.Write(data)
}

type stressFakeUDP struct {
	packets chan stressUDPPacket
	closed  chan struct{}
	once    sync.Once
	send    func([]byte, string) error
}

func newStressFakeUDP() *stressFakeUDP {
	return &stressFakeUDP{packets: make(chan stressUDPPacket, 32), closed: make(chan struct{})}
}

func (c *stressFakeUDP) Send(data []byte, addr string) error { return c.send(data, addr) }
func (c *stressFakeUDP) Receive() ([]byte, string, error) {
	select {
	case <-c.closed:
		return nil, "", io.EOF
	case p := <-c.packets:
		return p.data, p.addr, p.err
	}
}
func (c *stressFakeUDP) Close() error { c.once.Do(func() { close(c.closed) }); return nil }

func TestStressUDPTransfer(t *testing.T) {
	for _, tc := range []struct {
		name           string
		count, replies int
		wantErr        string
	}{
		{"all", 10, 10, ""},
		{"reordered", 10, 10, ""},
		{"20_percent_loss", 10, 8, ""},
		{"excessive_loss", 10, 7, "UDP loss"},
		{"all_lost", 10, 0, "received 0/10"},
		{"small_count_rounds_up", 1, 0, "minimum 1"},
		{"duplicates_do_not_mask_loss", 10, 7, "UDP loss"},
		{"corrupt", 10, 10, "payload mismatch"},
		{"wrong_session", 10, 10, "payload mismatch"},
		{"wrong_address", 10, 10, "source address"},
		{"wrong_length", 10, 10, "payload size"},
		{"invalid_sequence", 10, 10, "invalid packet number"},
		{"send_error", 10, 10, "send packet 0"},
		{"receive_error", 10, 10, "receive:"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
			deadlineCtx, stop := context.WithTimeout(t.Context(), 2*time.Second)
			defer stop()
			ctx, cancel := context.WithCancelCause(deadlineCtx)
			defer cancel(nil)
			conn := newStressFakeUDP()
			var pending []stressUDPPacket
			conn.send = func(data []byte, addr string) error {
				sequence := int(binary.BigEndian.Uint64(data))
				if sequence >= tc.replies {
					return nil
				}
				p := stressUDPPacket{data: data, addr: addr}
				switch tc.name {
				case "reordered":
					pending = append(pending, p)
					if sequence == tc.count-1 {
						for i := len(pending) - 1; i >= 0; i-- {
							conn.packets <- pending[i]
						}
					}
					return nil
				case "send_error":
					return errors.New("injected send failure")
				case "receive_error":
					p.err = io.ErrUnexpectedEOF
				case "corrupt":
					data[len(data)-1] ^= 0xff
				case "wrong_session":
					p.data = stressUDPPayload(len(data), 999, sequence)
				case "wrong_address":
					p.addr = "wrong:1234"
				case "wrong_length":
					p.data = data[:len(data)-1]
				case "invalid_sequence":
					binary.BigEndian.PutUint64(data, uint64(tc.count))
				}
				conn.packets <- p
				if tc.name == "duplicates_do_not_mask_loss" {
					conn.packets <- p
				}
				return nil
			}
			stats, err := stressUDPTransfer(ctx, cancel, conn, "127.0.0.1:1234", 100, tc.count, 42, 20*time.Millisecond, func(ctx context.Context) error { return ctx.Err() })
			if tc.wantErr == "" {
				require.NoError(t, err)
				require.Equal(t, tc.count, stats.sent)
				require.Equal(t, tc.replies, stats.received)
			} else {
				require.ErrorContains(t, err, tc.wantErr)
			}
			require.NotErrorIs(t, err, context.DeadlineExceeded, "loss must finish at the drain deadline, not the case timeout")
		})
	}
}

func TestStressUDPCancelsBlockedSender(t *testing.T) {
	defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
	ctx, cancel := context.WithCancelCause(t.Context())
	defer cancel(nil)
	conn := newStressFakeUDP()
	conn.send = func([]byte, string) error {
		// Receiving a malformed reply must cancel the transport before joining Send.
		conn.packets <- stressUDPPacket{data: []byte{1}, addr: "echo"}
		<-ctx.Done()
		return context.Cause(ctx)
	}
	_, err := stressUDPTransfer(ctx, cancel, conn, "echo", 100, 10, 1, time.Second, func(context.Context) error { return nil })
	require.ErrorContains(t, err, "payload size")
}

func TestStressUDPCancelsPacingAndReceive(t *testing.T) {
	defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
	ctx, cancel := context.WithCancelCause(t.Context())
	defer cancel(nil)
	conn := newStressFakeUDP()
	conn.send = func([]byte, string) error { return errors.New("unexpected send") }
	failure := errors.New("injected cancellation")
	_, err := stressUDPTransfer(ctx, cancel, conn, "echo", 100, 10, 1, time.Second, func(ctx context.Context) error {
		cancel(failure)
		<-ctx.Done()
		return context.Cause(ctx)
	})
	require.ErrorIs(t, err, failure)
}

func TestStressWorkersJoinOnFailure(t *testing.T) {
	defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
	ctx, cancel := context.WithCancelCause(t.Context())
	defer cancel(nil)
	failure := errors.New("injected failure")
	err := runStressWorkers(ctx, cancel, 3, 8, func(ctx context.Context, id int) error {
		if id == 0 {
			return failure
		}
		<-ctx.Done()
		return context.Cause(ctx)
	})
	require.ErrorIs(t, err, failure)
	require.ErrorContains(t, err, "iteration 0 worker 0")
}

func TestStressWorkersKeepDeadlineDetails(t *testing.T) {
	ctx, cancel := context.WithCancelCause(t.Context())
	defer cancel(nil)
	err := runStressWorkers(ctx, cancel, 1, 1, func(context.Context, int) error {
		cancel(context.DeadlineExceeded)
		return io.ErrUnexpectedEOF
	})
	require.ErrorIs(t, err, context.DeadlineExceeded)
	require.ErrorIs(t, err, io.ErrUnexpectedEOF)
	require.ErrorContains(t, err, "iteration 0 worker 0")
}
