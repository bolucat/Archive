package localdns

import (
	"context"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

type LocalTransport interface {
	dns.Transport
	IsLocalTransport()
}

var transportInstance LocalTransport

func init() {
	SetTransport(nil)
}

func SetTransport(transport LocalTransport) {
	if transport == nil {
		transport = &DefaultTransport{
			&net.Resolver{
				PreferGo: false,
			},
		}
	}
	transportInstance = transport
}

type transportWrapper struct {
	LocalTransport
}

func Transport() dns.Transport {
	return &transportWrapper{transportInstance}
}

var _ dns.Transport = (*DefaultTransport)(nil)

type DefaultTransport struct {
	*net.Resolver
}

func (t *DefaultTransport) Type() dns.TransportType {
	return dns.TransportTypeLookup
}

func (t *DefaultTransport) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	var network string
	switch strategy {
	case dns.QueryStrategy_USE_IP4:
		network = "ip4"
	case dns.QueryStrategy_USE_IP6:
		network = "ip6"
	default:
		network = "ip"
	}
	return t.LookupIP(ctx, network, domain)
}

func (t *DefaultTransport) Write(context.Context, *dnsmessage.Message) error {
	return common.ErrNoClue
}

func (t *DefaultTransport) Exchange(context.Context, *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (t *DefaultTransport) ExchangeRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}

func (t *DefaultTransport) IsLocalTransport() {
}

func (t *DefaultTransport) Close() error {
	return nil
}
