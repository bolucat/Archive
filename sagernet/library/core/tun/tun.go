package tun

import (
	"io"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
)

type Tun interface {
	io.Closer
}

type Handler interface {
	NewConnection(source net.Destination, destination net.Destination, conn net.Conn)
	NewPacket(source net.Destination, destination net.Destination, data *buf.Buffer, writeBack func([]byte, *net.UDPAddr) (int, error), closer io.Closer)
	NewPingPacket(source net.Destination, destination net.Destination, message *buf.Buffer, writeBack func([]byte) error, closer io.Closer) bool
}
