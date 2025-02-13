package outbound

import (
	"context"
	"io"
	"net"
	"time"

	"github.com/MerlinKodo/clash-rev/common/buf"
	"github.com/MerlinKodo/clash-rev/component/dialer"
	C "github.com/MerlinKodo/clash-rev/constant"
)

type Reject struct {
	*Base
}

type RejectOption struct {
	Name string `proxy:"name"`
}

// DialContext implements C.ProxyAdapter
func (r *Reject) DialContext(ctx context.Context, metadata *C.Metadata, opts ...dialer.Option) (C.Conn, error) {
	return NewConn(nopConn{}, r), nil
}

// ListenPacketContext implements C.ProxyAdapter
func (r *Reject) ListenPacketContext(ctx context.Context, metadata *C.Metadata, opts ...dialer.Option) (C.PacketConn, error) {
	return newPacketConn(nopPacketConn{}, r), nil
}

func NewRejectWithOption(option RejectOption) *Reject {
	return &Reject{
		Base: &Base{
			name: option.Name,
			tp:   C.Direct,
			udp:  true,
		},
	}
}

func NewReject() *Reject {
	return &Reject{
		Base: &Base{
			name:   "REJECT",
			tp:     C.Reject,
			udp:    true,
			prefer: C.DualStack,
		},
	}
}

func NewPass() *Reject {
	return &Reject{
		Base: &Base{
			name:   "PASS",
			tp:     C.Pass,
			udp:    true,
			prefer: C.DualStack,
		},
	}
}

type nopConn struct{}

func (rw nopConn) Read(b []byte) (int, error) {
	return 0, io.EOF
}

func (rw nopConn) ReadBuffer(buffer *buf.Buffer) error {
	return io.EOF
}

func (rw nopConn) Write(b []byte) (int, error) {
	return 0, io.EOF
}

func (rw nopConn) WriteBuffer(buffer *buf.Buffer) error {
	return io.EOF
}

func (rw nopConn) Close() error                     { return nil }
func (rw nopConn) LocalAddr() net.Addr              { return nil }
func (rw nopConn) RemoteAddr() net.Addr             { return nil }
func (rw nopConn) SetDeadline(time.Time) error      { return nil }
func (rw nopConn) SetReadDeadline(time.Time) error  { return nil }
func (rw nopConn) SetWriteDeadline(time.Time) error { return nil }

var udpAddrIPv4Unspecified = &net.UDPAddr{IP: net.IPv4zero, Port: 0}

type nopPacketConn struct{}

func (npc nopPacketConn) WriteTo(b []byte, addr net.Addr) (n int, err error) { return len(b), nil }
func (npc nopPacketConn) ReadFrom(b []byte) (int, net.Addr, error)           { return 0, nil, io.EOF }
func (npc nopPacketConn) WaitReadFrom() ([]byte, func(), net.Addr, error) {
	return nil, nil, nil, io.EOF
}
func (npc nopPacketConn) Close() error                     { return nil }
func (npc nopPacketConn) LocalAddr() net.Addr              { return udpAddrIPv4Unspecified }
func (npc nopPacketConn) SetDeadline(time.Time) error      { return nil }
func (npc nopPacketConn) SetReadDeadline(time.Time) error  { return nil }
func (npc nopPacketConn) SetWriteDeadline(time.Time) error { return nil }
