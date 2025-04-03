package gun

import (
	"golang.org/x/net/http2"
	"net"
)

type TransportWrap struct {
	*http2.Transport
}

type netAddr struct {
	remoteAddr net.Addr
	localAddr  net.Addr
}

func (addr netAddr) RemoteAddr() net.Addr {
	return addr.remoteAddr
}

func (addr netAddr) LocalAddr() net.Addr {
	return addr.localAddr
}
