package dns

import (
	"context"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
)

var _ dns.Transport = (*UDPTransport)(nil)

type UDPTransport struct {
	dispatcher *messageDispatcher
}

func (t *UDPTransport) Close() error {
	return t.dispatcher.Close()
}

func NewUDPTransport(ctx *transportContext, dispatcher routing.Dispatcher) *UDPTransport {
	return &UDPTransport{
		NewDispatcher(ctx, dispatcher, ctx.destination, ctx.writeBackRaw),
	}
}

func NewUDPLocalTransport(ctx *transportContext) *UDPTransport {
	return &UDPTransport{
		NewLocalDispatcher(ctx, ctx.destination, ctx.writeBackRaw),
	}
}

func (t *UDPTransport) Type() dns.TransportType {
	return dns.TransportTypeDefault
}

func (t *UDPTransport) Write(ctx context.Context, message *dnsmessage.Message) error {
	packed, err := message.Pack()
	if err != nil {
		return newError("failed to pack dns query").Base(err)
	}
	return task.Run(ctx, func() error {
		return t.dispatcher.Write(ctx, buf.MultiBuffer{buf.FromBytes(packed)})
	})
}

func (t *UDPTransport) Exchange(context.Context, *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (t *UDPTransport) ExchangeRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}

func (t *UDPTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
