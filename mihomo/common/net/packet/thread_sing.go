package packet

import (
	"github.com/metacubex/sing/common/buf"
	M "github.com/metacubex/sing/common/metadata"
	N "github.com/metacubex/sing/common/network"
)

type threadSafeSingPacketConn struct {
	*threadSafePacketConn
	singPacketConn SingPacketConn
}

var _ N.NetPacketConn = (*threadSafeSingPacketConn)(nil)

func (c *threadSafeSingPacketConn) WritePacket(buffer *buf.Buffer, destination M.Socksaddr) error {
	c.access.Lock()
	defer c.access.Unlock()
	return c.singPacketConn.WritePacket(buffer, destination)
}

func (c *threadSafeSingPacketConn) ReadPacket(buffer *buf.Buffer) (destination M.Socksaddr, err error) {
	return c.singPacketConn.ReadPacket(buffer)
}
