package tun

import (
	v2rayNet "github.com/xtls/xray-core/common/net"
	"io"
	"net"
)

type Tun interface {
	io.Closer
}

type Handler interface {
	NewConnection(source v2rayNet.Destination, destination v2rayNet.Destination, conn net.Conn)
	NewPacket(source v2rayNet.Destination, destination v2rayNet.Destination, data []byte, writeBack func([]byte, *net.UDPAddr) (int, error), closer io.Closer)
}
