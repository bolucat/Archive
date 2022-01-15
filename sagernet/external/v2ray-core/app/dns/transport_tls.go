package dns

import (
	"context"
	"crypto/tls"
	"encoding/binary"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
)

var _ dns.Transport = (*TLSTransport)(nil)

type TLSTransport struct {
	dispatcher *messageDispatcher
}

func NewTLSTransport(ctx *transportContext, dispatcher routing.Dispatcher) *TLSTransport {
	return &TLSTransport{
		dispatcher: NewRawDispatcher(ctx.ctx, func() (net.Conn, error) {
			link, _ := dispatcher.Dispatch(ctx.ctx, ctx.destination)
			conn := buf.NewConnection(buf.ConnectionOutputMulti(link.Reader), buf.ConnectionInputMulti(link.Writer))
			return tls.Client(conn, &tls.Config{
				ServerName: ctx.destination.Address.String(),
			}), nil
		}, ctx.destination, ctx.writeBackRawTCP),
	}
}

func NewTLSLocalTransport(ctx *transportContext) *TLSTransport {
	return &TLSTransport{
		dispatcher: NewRawLocalDispatcher(ctx.ctx, func(conn net.Conn) (net.Conn, error) {
			return tls.Client(conn, &tls.Config{
				ServerName: ctx.destination.Address.String(),
			}), nil
		}, ctx.destination, ctx.writeBackRawTCP),
	}
}

func (t *TLSTransport) SupportRaw() bool {
	return true
}

func (t *TLSTransport) WriteMessage(ctx context.Context, message *dnsmessage.Message) error {
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

func (t *TLSTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
