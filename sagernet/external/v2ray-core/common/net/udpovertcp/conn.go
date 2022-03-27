package udpovertcp

import (
	"encoding/binary"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/transport/pipe"
)

type ServerConn struct {
	net.Conn
	upstream net.PacketConn
}

func NewServerConn(packetConn net.PacketConn) *ServerConn {
	c := &ServerConn{upstream: packetConn}
	reader, writer := pipe.New()
	c.Conn = buf.NewConnection(buf.ConnectionInputMulti(writer), buf.ConnectionOutputMulti(c), buf.ConnectionOnClose(packetConn))
	go c.loopInput(NewBufferedReader(reader))
	return c
}

func (c *ServerConn) ReadMultiBuffer() (buf.MultiBuffer, error) {
	buffer := buf.New()
	n, addr, err := c.upstream.ReadFrom(buffer.Extend(buf.Size))
	if err != nil {
		buffer.Release()
		return nil, err
	}
	buffer.Resize(0, int32(n))
	header := buf.New()
	endpoint := net.DestinationFromAddr(addr)
	addrParser.WriteAddressPort(header, endpoint.Address, endpoint.Port)
	binary.Write(header, binary.BigEndian, uint16(buffer.Len()))
	return buf.MultiBuffer{header, buffer}, nil
}

func (c *ServerConn) loopInput(reader buf.Reader) {
	for {
		mb, err := reader.ReadMultiBuffer()
		if err != nil {
			break
		}
		for _, buffer := range mb {
			if buffer.Endpoint == nil {
				panic("nil udp endpoint")
			}
			_, err = c.upstream.WriteTo(buffer.Bytes(), buffer.Endpoint.UDPAddr())
			if err != nil {
				break
			}
		}
	}
	c.Close()
}

func (c *ServerConn) Interrupt() {
}
