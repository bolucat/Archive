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

func NewTCPTransport(ctx *transportContext, dispatcher routing.Dispatcher) *TCPTransport {
	return &TCPTransport{
		NewDispatcher(ctx.ctx, dispatcher, ctx.destination, ctx.writeBackRawTCP),
	}
}

func NewTCPLocalTransport(ctx *transportContext) *TCPTransport {
	return &TCPTransport{
		NewLocalDispatcher(ctx.ctx, ctx.destination, ctx.writeBackRawTCP),
	}
}

func (t *TCPTransport) SupportRaw() bool {
	return true
}

func (t *TCPTransport) WriteMessage(ctx context.Context, message *dnsmessage.Message) error {
	packed, err := message.Pack()
	if err != nil {
		return newError("failed to pack dns query").Base(err)
	}

	buffer := buf.New()
	binary.Write(buffer, binary.BigEndian, uint16(len(packed)))
	buffer.Write(packed)
	return task.Run(ctx, func() error {
		return t.dispatcher.Write(buffer)
	})
}

func (t *TCPTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
