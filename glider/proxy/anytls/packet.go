package anytls

import (
	"encoding/binary"
	"errors"
	"io"
	"net"
	"time"

	"github.com/nadoo/glider/pkg/pool"
	"github.com/nadoo/glider/pkg/socks"
)

const uotV2MagicHost = "sp.v2.udp-over-tcp.arpa"

// uotPacketConn carries UDP packets over an AnyTLS stream using sing-box
// udp-over-tcp v2 connect format.
type uotPacketConn struct {
	net.Conn
	target socks.Addr
}

func newUOTPacketConn(c net.Conn, target socks.Addr) *uotPacketConn {
	return &uotPacketConn{Conn: c, target: target}
}

func (pc *uotPacketConn) ReadFrom(b []byte) (int, net.Addr, error) {
	if len(b) < 2 {
		return 0, pc.target, errors.New("buf size is not enough")
	}

	if _, err := io.ReadFull(pc.Conn, b[:2]); err != nil {
		return 0, pc.target, err
	}
	length := int(binary.BigEndian.Uint16(b[:2]))
	if len(b) < length {
		return 0, pc.target, errors.New("buf size is not enough")
	}

	n, err := io.ReadFull(pc.Conn, b[:length])
	return n, pc.target, err
}

func (pc *uotPacketConn) WriteTo(b []byte, addr net.Addr) (int, error) {
	buf := pool.GetBytesBuffer()
	defer pool.PutBytesBuffer(buf)

	var head [2]byte
	binary.BigEndian.PutUint16(head[:], uint16(len(b)))
	buf.Write(head[:])
	buf.Write(b)

	n, err := pc.Write(buf.Bytes())
	if n > 2 {
		return n - 2, err
	}
	return 0, err
}

func (pc *uotPacketConn) SetDeadline(t time.Time) error {
	return pc.Conn.SetDeadline(t)
}

func (pc *uotPacketConn) SetReadDeadline(t time.Time) error {
	return pc.Conn.SetReadDeadline(t)
}

func (pc *uotPacketConn) SetWriteDeadline(t time.Time) error {
	return pc.Conn.SetWriteDeadline(t)
}

func writeUOTV2Request(w io.Writer, target socks.Addr) error {
	if target == nil {
		return errors.New("invalid target address")
	}

	buf := pool.GetBytesBuffer()
	defer pool.PutBytesBuffer(buf)

	buf.WriteByte(1) // connect stream format
	buf.Write(target)
	_, err := w.Write(buf.Bytes())
	return err
}

func readUOTV2Request(r io.Reader) (socks.Addr, error) {
	var connect [1]byte
	if _, err := io.ReadFull(r, connect[:]); err != nil {
		return nil, err
	}
	if connect[0] != 1 {
		return nil, errors.New("udp-over-tcp v2 non-connect format is not supported")
	}
	return socks.ReadAddr(r)
}
