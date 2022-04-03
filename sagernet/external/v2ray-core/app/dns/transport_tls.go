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

func (t *TLSTransport) Close() error {
	return t.dispatcher.Close()
}

func NewTLSTransport(ctx *transportContext, dispatcher routing.Dispatcher) *TLSTransport {
	return &TLSTransport{
		NewRawDispatcher(func() (net.Conn, error) {
			link, _ := dispatcher.Dispatch(ctx.newContext(), ctx.destination)
			conn := buf.NewConnection(buf.ConnectionOutputMulti(link.Reader), buf.ConnectionInputMulti(link.Writer))
			return tls.Client(conn, &tls.Config{
				ServerName: ctx.destination.Address.String(),
			}), nil
		}, ctx.destination, ctx.writeBackRawTCP),
	}
}

func NewTLSLocalTransport(ctx *transportContext) *TLSTransport {
	return &TLSTransport{
		NewRawLocalDispatcher(ctx, func(conn net.Conn) (net.Conn, error) {
			return tls.Client(conn, &tls.Config{
				ServerName: ctx.destination.Address.String(),
			}), nil
		}, ctx.destination, ctx.writeBackRawTCP),
	}
}

func (t *TLSTransport) Type() dns.TransportType {
	return dns.TransportTypeDefault
}

func (t *TLSTransport) Write(ctx context.Context, message *dnsmessage.Message) error {
	packed, err := message.Pack()
	if err != nil {
		return newError("failed to pack dns query").Base(err)
	}

	header := make([]byte, 2)
	binary.BigEndian.PutUint16(header, uint16(len(packed)))
	return task.Run(ctx, func() error {
		return t.dispatcher.Write(ctx, buf.MultiBuffer{buf.FromBytes(header), buf.FromBytes(packed)})
	})
}

func (t *TLSTransport) Exchange(context.Context, *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (t *TLSTransport) ExchangeRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}

func (t *TLSTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
