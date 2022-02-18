package dns

import (
	"context"
	"encoding/binary"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
)

var _ dns.Transport = (*TCPTransport)(nil)

type TCPTransport struct {
	dispatcher *messageDispatcher
}

func (t *TCPTransport) Close() error {
	return t.dispatcher.Close()
}

func NewTCPTransport(ctx *transportContext, dispatcher routing.Dispatcher) *TCPTransport {
	return &TCPTransport{
		NewDispatcher(ctx, dispatcher, ctx.destination, ctx.writeBackRawTCP),
	}
}

func NewTCPLocalTransport(ctx *transportContext) *TCPTransport {
	return &TCPTransport{
		NewLocalDispatcher(ctx, ctx.destination, ctx.writeBackRawTCP),
	}
}

func (t *TCPTransport) Type() dns.TransportType {
	return dns.TransportTypeDefault
}

func (t *TCPTransport) Write(ctx context.Context, message *dnsmessage.Message) error {
	packed, err := message.Pack()
	if err != nil {
		return newError("failed to pack dns query").Base(err)
	}

	buffer := buf.New()
	binary.Write(buffer, binary.BigEndian, uint16(len(packed)))
	buffer.Write(packed)
	return task.Run(ctx, func() error {
		return t.dispatcher.Write(ctx, buffer)
	})
}

func (t *TCPTransport) Exchange(context.Context, *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (t *TCPTransport) ExchangeRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}

func (t *TCPTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
