package loopback

import (
	"context"

	core "github.com/v2fly/v2ray-core/v5"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

type Loopback struct {
	inboundTag string
	dispatcher routing.Dispatcher
}

func (l *Loopback) Process(ctx context.Context, link *transport.Link, _ internet.Dialer) error {
	outbound := session.OutboundFromContext(ctx)
	if outbound == nil || !outbound.Target.IsValid() {
		return newError("target not specified.")
	}
	destination := outbound.Target
	newError("opening connection to ", destination).WriteToLog(session.ExportIDToError(ctx))

	inbound := session.InboundFromContext(ctx)
	if inbound == nil {
		inbound = new(session.Inbound)
		ctx = session.ContextWithInbound(ctx, inbound)
	}
	inbound.Tag = l.inboundTag
	content := session.ContentFromContext(ctx)
	if content == nil {
		content = new(session.Content)
		ctx = session.ContextWithContent(ctx, content)
	}
	content.SkipDNSResolve = true
	return l.dispatcher.DispatchLink(ctx, destination, link)
}

func (l *Loopback) init(config *Config, dispatcher routing.Dispatcher) error {
	l.dispatcher = dispatcher
	l.inboundTag = config.InboundTag
	return nil
}

func init() {
	common.Must(common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		l := new(Loopback)
		err := core.RequireFeatures(ctx, func(dispatcher routing.Dispatcher) error {
			return l.init(config.(*Config), dispatcher)
		})
		return l, err
	}))
}
