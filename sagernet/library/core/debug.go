package libcore

import (
	"net/http"
	_ "net/http/pprof"
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
	closeIgnore(d.server)
}
