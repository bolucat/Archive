package inbound

import (
	"context"
	"net"
	"os"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/mux"
	"github.com/sagernet/sing-box/common/uot"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing-shadowsocks"
	"github.com/sagernet/sing-shadowsocks/shadowaead"
	"github.com/sagernet/sing-shadowsocks/shadowaead_2022"
	"github.com/sagernet/sing/common"
	"github.com/sagernet/sing/common/buf"
	E "github.com/sagernet/sing/common/exceptions"
	N "github.com/sagernet/sing/common/network"
	"github.com/sagernet/sing/common/ntp"
)

func NewShadowsocks(ctx context.Context, router adapter.Router, logger log.ContextLogger, tag string, options option.ShadowsocksInboundOptions) (adapter.Inbound, error) {
	if len(options.Users) > 0 && len(options.Destinations) > 0 {
		return nil, E.New("users and destinations options must not be combined")
	}
	if len(options.Users) > 0 {
		return newShadowsocksMulti(ctx, router, logger, tag, options)
	} else if len(options.Destinations) > 0 {
		return newShadowsocksRelay(ctx, router, logger, tag, options)
	} else {
		return newShadowsocks(ctx, router, logger, tag, options)
	}
}

var (
	_ adapter.Inbound           = (*Shadowsocks)(nil)
	_ adapter.InjectableInbound = (*Shadowsocks)(nil)
)

type Shadowsocks struct {
	myInboundAdapter
	service shadowsocks.Service
}

func newShadowsocks(ctx context.Context, router adapter.Router, logger log.ContextLogger, tag string, options option.ShadowsocksInboundOptions) (*Shadowsocks, error) {
	inbound := &Shadowsocks{
		myInboundAdapter: myInboundAdapter{
			protocol:      C.TypeShadowsocks,
			network:       options.Network.Build(),
			ctx:           ctx,
			router:        uot.NewRouter(router, logger),
			logger:        logger,
			tag:           tag,
			listenOptions: options.ListenOptions,
		},
	}

	inbound.connHandler = inbound
	inbound.packetHandler = inbound
	var err error
	inbound.router, err = mux.NewRouterWithOptions(inbound.router, logger, common.PtrValueOrDefault(options.Multiplex))
	if err != nil {
		return nil, err
	}

	var udpTimeout time.Duration
	if options.UDPTimeout != 0 {
		udpTimeout = time.Duration(options.UDPTimeout)
	} else {
		udpTimeout = C.UDPTimeout
	}
	switch {
	case options.Method == shadowsocks.MethodNone:
		inbound.service = shadowsocks.NewNoneService(int64(udpTimeout.Seconds()), inbound.upstreamContextHandler())
	case common.Contains(shadowaead.List, options.Method):
		inbound.service, err = shadowaead.NewService(options.Method, nil, options.Password, int64(udpTimeout.Seconds()), inbound.upstreamContextHandler())
	case common.Contains(shadowaead_2022.List, options.Method):
		inbound.service, err = shadowaead_2022.NewServiceWithPassword(options.Method, options.Password, int64(udpTimeout.Seconds()), inbound.upstreamContextHandler(), ntp.TimeFuncFromContext(ctx))
	default:
		err = E.New("unsupported method: ", options.Method)
	}
	inbound.packetUpstream = inbound.service
	return inbound, err
}

func (h *Shadowsocks) NewConnection(ctx context.Context, conn net.Conn, metadata adapter.InboundContext) error {
	return h.service.NewConnection(adapter.WithContext(log.ContextWithNewID(ctx), &metadata), conn, adapter.UpstreamMetadata(metadata))
}

func (h *Shadowsocks) NewPacket(ctx context.Context, conn N.PacketConn, buffer *buf.Buffer, metadata adapter.InboundContext) error {
	return h.service.NewPacket(adapter.WithContext(ctx, &metadata), conn, buffer, adapter.UpstreamMetadata(metadata))
}

func (h *Shadowsocks) NewPacketConnection(ctx context.Context, conn N.PacketConn, metadata adapter.InboundContext) error {
	return os.ErrInvalid
}
