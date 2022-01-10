package wireguard

import (
	"io"
	"time"

	"github.com/v2fly/v2ray-core/v5/common/net"
)

type pingConnWrapper struct {
	outbound func(message []byte, addr net.Addr) (int, error)
}

func (c *pingConnWrapper) ReadFrom([]byte) (n int, addr net.Addr, err error) {
	return 0, nil, io.EOF
}

func (c *pingConnWrapper) WriteTo(p []byte, addr net.Addr) (n int, err error) {
	return c.outbound(p, addr)
}

func (c *pingConnWrapper) Close() error {
	return nil
}

func (c *pingConnWrapper) LocalAddr() net.Addr {
	return nil
}

func (c *pingConnWrapper) SetDeadline(t time.Time) error {
	return nil
}

func (c *pingConnWrapper) SetReadDeadline(t time.Time) error {
	return nil
}

func (c *pingConnWrapper) SetWriteDeadline(t time.Time) error {
	return nil
}
