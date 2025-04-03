package gun

import (
	"sync"
	"unsafe"

	"golang.org/x/net/http2"
)

type clientConnPool struct {
	t     *http2.Transport
	mu    sync.Mutex
	conns map[string][]*http2.ClientConn // key is host:port
}

type efaceWords struct {
	typ  unsafe.Pointer
	data unsafe.Pointer
}

func (tw *TransportWrap) Close() error {
	connPool := transportConnPool(tw.Transport)
	p := (*clientConnPool)((*efaceWords)(unsafe.Pointer(&connPool)).data)
	p.mu.Lock()
	defer p.mu.Unlock()
	for _, vv := range p.conns {
		for _, cc := range vv {
			cc.Close()
		}
	}
	return nil
}

//go:linkname transportConnPool golang.org/x/net/http2.(*Transport).connPool
func transportConnPool(t *http2.Transport) http2.ClientConnPool
