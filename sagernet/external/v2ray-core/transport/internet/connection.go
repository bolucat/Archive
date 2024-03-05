package internet

import (
	"net"

	B "github.com/sagernet/sing/common/buf"
	M "github.com/sagernet/sing/common/metadata"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/features/stats"
)

type Connection interface {
	net.Conn
}

type AbstractPacketConnReader interface {
	ReadFrom(p []byte) (n int, addr net.Addr, err error)
}

type AbstractPacketConnWriter interface {
	WriteTo(p []byte, addr net.Addr) (n int, err error)
}

type AbstractPacketConn interface {
	AbstractPacketConnReader
	AbstractPacketConnWriter
	common.Closable
}

type PacketConn interface {
	AbstractPacketConn
	net.PacketConn
}

type StatCounterConn struct {
	Connection
	ReadCounter  stats.Counter
	WriteCounter stats.Counter
}

func (c *StatCounterConn) Read(b []byte) (int, error) {
	nBytes, err := c.Connection.Read(b)
	if c.ReadCounter != nil {
		c.ReadCounter.Add(int64(nBytes))
	}

	return nBytes, err
}

func (c *StatCounterConn) Write(b []byte) (int, error) {
	nBytes, err := c.Connection.Write(b)
	if c.WriteCounter != nil {
		c.WriteCounter.Add(int64(nBytes))
	}
	return nBytes, err
}

type StatCounterPacketConn struct {
	net.PacketConn
	ReadCounter  stats.Counter
	WriteCounter stats.Counter
}

func (c *StatCounterPacketConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	n, addr, err = c.PacketConn.ReadFrom(p)
	if c.ReadCounter != nil {
		c.ReadCounter.Add(int64(n))
	}
	return
}

func (c *StatCounterPacketConn) WriteTo(p []byte, addr net.Addr) (n int, err error) {
	n, err = c.PacketConn.WriteTo(p, addr)
	if c.WriteCounter != nil {
		c.WriteCounter.Add(int64(n))
	}
	return
}

func (c *StatCounterPacketConn) ReadPacket(buffer *B.Buffer) (M.Socksaddr, error) {
	_, addr, err := buffer.ReadPacketFrom(c)
	if err != nil {
		return M.Socksaddr{}, err
	}
	return M.SocksaddrFromNet(addr), nil
}

func (c *StatCounterPacketConn) WritePacket(buffer *B.Buffer, destination M.Socksaddr) error {
	_, err := c.WriteTo(buffer.Bytes(), destination.UDPAddr())
	return err
}

func (c *StatCounterPacketConn) RemoteAddr() net.Addr {
	return nil
}
