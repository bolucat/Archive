//go:build !disable_debug

package libcore

import (
	"net/http"
	_ "net/http/pprof"

	"libcore/comm"
)

type DebugInstance struct {
	server *http.Server
}

func NewDebugInstance() *DebugInstance {
	s := &http.Server{
		Addr: "0.0.0.0:8964",
	}
	go func() {
		_ = s.ListenAndServe()
	}()
	return &DebugInstance{s}
}

func (d *DebugInstance) Close() {
	comm.CloseIgnore(d.server)
}
