package anytls

import (
	"errors"
	"io"
	"net"
	"sync"
	"time"
)

var errStreamClosed = errors.New("stream closed")

type stream struct {
	id uint32
	s  *session

	in       chan []byte
	readBuf  []byte
	closeIn  sync.Once
	closeOut sync.Once

	mu     sync.Mutex
	closed bool
}

func newStream(id uint32, s *session) *stream {
	return &stream{id: id, s: s, in: make(chan []byte, 32)}
}

func (st *stream) Read(p []byte) (int, error) {
	for len(st.readBuf) == 0 {
		b, ok := <-st.in
		if !ok {
			return 0, io.EOF
		}
		st.readBuf = b
	}
	n := copy(p, st.readBuf)
	st.readBuf = st.readBuf[n:]
	return n, nil
}

func (st *stream) Write(p []byte) (int, error) {
	st.mu.Lock()
	closed := st.closed
	st.mu.Unlock()
	if closed {
		return 0, errStreamClosed
	}
	written := 0
	for len(p) > 0 {
		n := min(len(p), maxFrameData)
		if err := st.s.writeFrame(frame{command: cmdPSH, streamID: st.id, data: p[:n]}); err != nil {
			return written, err
		}
		written += n
		p = p[n:]
	}
	return written, nil
}

func (st *stream) Close() error {
	st.mu.Lock()
	already := st.closed
	st.closed = true
	st.mu.Unlock()
	if !already {
		st.closeOut.Do(func() {
			_ = st.s.writeFrame(frame{command: cmdFIN, streamID: st.id})
		})
		st.s.removeStream(st.id)
	}
	return nil
}

func (st *stream) closeRead() {
	st.closeIn.Do(func() { close(st.in) })
}

func (st *stream) push(data []byte) {
	cp := make([]byte, len(data))
	copy(cp, data)
	select {
	case st.in <- cp:
	case <-st.s.done:
	}
}

func (st *stream) LocalAddr() net.Addr                { return st.s.conn.LocalAddr() }
func (st *stream) RemoteAddr() net.Addr               { return st.s.conn.RemoteAddr() }
func (st *stream) SetDeadline(t time.Time) error      { return st.s.conn.SetDeadline(t) }
func (st *stream) SetReadDeadline(t time.Time) error  { return st.s.conn.SetReadDeadline(t) }
func (st *stream) SetWriteDeadline(t time.Time) error { return st.s.conn.SetWriteDeadline(t) }
