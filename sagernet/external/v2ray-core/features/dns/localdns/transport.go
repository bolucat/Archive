package localdns

import (
	"context"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

type LookupFunc func(ctx context.Context, network string, host string) ([]net.IP, error)

var LocalLookupFunc LookupFunc

func init() {
	SetLocalLookupFunc(nil)
}

func SetLocalLookupFunc(lookupFunc func(ctx context.Context, network string, host string) ([]net.IP, error)) {
	if lookupFunc == nil {
		resolver := &net.Resolver{PreferGo: false}
		LocalLookupFunc = resolver.LookupIP
	} else {
		LocalLookupFunc = lookupFunc
	}
}

var transportInstance dns.Transport = &LocalTransport{}

type LocalTransport struct{}

func NewLocalTransport() dns.Transport {
	return transportInstance
}

func (t *LocalTransport) SupportRaw() bool {
	return false
}

func (t *LocalTransport) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	var network string
	switch strategy {
	case dns.QueryStrategy_USE_IP4:
		network = "ip4"
	case dns.QueryStrategy_USE_IP6:
		network = "ip6"
	default:
		network = "ip"
	}
	return LocalLookupFunc(ctx, network, domain)
}

func (t *LocalTransport) WriteMessage(context.Context, *dnsmessage.Message) error {
	return common.ErrNoClue
}
