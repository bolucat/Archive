package option

import (
	"context"

	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/json"
	"github.com/sagernet/sing/common/json/badjson"
	M "github.com/sagernet/sing/common/metadata"
	"github.com/sagernet/sing/service"
)

type OutboundOptionsRegistry interface {
	CreateOptions(outboundType string) (any, bool)
}

type _Outbound struct {
	Type    string `json:"type"`
	Tag     string `json:"tag,omitempty"`
	Options any    `json:"-"`
}

type Outbound _Outbound

func (h *Outbound) MarshalJSONContext(ctx context.Context) ([]byte, error) {
	return badjson.MarshallObjectsContext(ctx, (*_Outbound)(h), h.Options)
}

func (h *Outbound) UnmarshalJSONContext(ctx context.Context, content []byte) error {
	err := json.Unmarshal(content, (*_Outbound)(h))
	if err != nil {
		return err
	}
	registry := service.FromContext[OutboundOptionsRegistry](ctx)
	if registry == nil {
		return E.New("missing outbound options registry in context")
	}
	options, loaded := registry.CreateOptions(h.Type)
	if !loaded {
		return E.New("unknown outbound type: ", h.Type)
	}
	err = badjson.UnmarshallExcludedContext(ctx, content, (*_Outbound)(h), options)
	if err != nil {
		return err
	}
	h.Options = options
	return nil
}

type DialerOptionsWrapper interface {
	TakeDialerOptions() DialerOptions
	ReplaceDialerOptions(options DialerOptions)
}

type DialerOptions struct {
	Detour              string         `json:"detour,omitempty"`
	BindInterface       string         `json:"bind_interface,omitempty"`
	Inet4BindAddress    *ListenAddress `json:"inet4_bind_address,omitempty"`
	Inet6BindAddress    *ListenAddress `json:"inet6_bind_address,omitempty"`
	ProtectPath         string         `json:"protect_path,omitempty"`
	RoutingMark         uint32         `json:"routing_mark,omitempty"`
	ReuseAddr           bool           `json:"reuse_addr,omitempty"`
	ConnectTimeout      Duration       `json:"connect_timeout,omitempty"`
	TCPFastOpen         bool           `json:"tcp_fast_open,omitempty"`
	TCPMultiPath        bool           `json:"tcp_multi_path,omitempty"`
	UDPFragment         *bool          `json:"udp_fragment,omitempty"`
	UDPFragmentDefault  bool           `json:"-"`
	DomainStrategy      DomainStrategy `json:"domain_strategy,omitempty"`
	FallbackDelay       Duration       `json:"fallback_delay,omitempty"`
	IsWireGuardListener bool           `json:"-"`
}

func (o *DialerOptions) TakeDialerOptions() DialerOptions {
	return *o
}

func (o *DialerOptions) ReplaceDialerOptions(options DialerOptions) {
	*o = options
}

type ServerOptionsWrapper interface {
	TakeServerOptions() ServerOptions
	ReplaceServerOptions(options ServerOptions)
}

type ServerOptions struct {
	Server     string `json:"server"`
	ServerPort uint16 `json:"server_port"`
}

func (o ServerOptions) Build() M.Socksaddr {
	return M.ParseSocksaddrHostPort(o.Server, o.ServerPort)
}

func (o *ServerOptions) TakeServerOptions() ServerOptions {
	return *o
}

func (o *ServerOptions) ReplaceServerOptions(options ServerOptions) {
	*o = options
}
