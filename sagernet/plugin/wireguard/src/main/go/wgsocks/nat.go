package main

import (
	"gvisor.dev/gvisor/pkg/sync"
	"gvisor.dev/gvisor/pkg/tcpip/adapters/gonet"
)

type natTable struct {
	mapping sync.Map
}

func (t *natTable) Set(key string, conn *gonet.UDPConn) {
	t.mapping.Store(key, conn)
}

func (t *natTable) Get(key string) *gonet.UDPConn {
	item, exist := t.mapping.Load(key)
	if !exist {
		return nil
	}
	return item.(*gonet.UDPConn)
}

func (t *natTable) GetOrCreateLock(key string) (*sync.Cond, bool) {
	item, loaded := t.mapping.LoadOrStore(key, sync.NewCond(&sync.Mutex{}))
	return item.(*sync.Cond), loaded
}

func (t *natTable) Delete(key string) {
	t.mapping.Delete(key)
}
