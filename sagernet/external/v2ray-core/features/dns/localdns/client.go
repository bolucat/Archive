package localdns

import (
	"context"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

var _ dns.NewClient = (*LocalClient)(nil)

var instance dns.NewClient = &LocalClient{}

type LocalClient struct{}

func Client() dns.NewClient {
	return instance
}

func (c LocalClient) Type() interface{} {
	return dns.ClientType()
}

func (c LocalClient) Start() error {
	return nil
}

func (c LocalClient) Close() error {
	return nil
}

func (c *LocalClient) LookupIP(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.LookupDefault(ctx, domain)
}

func (c *LocalClient) LookupIPv4(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP4)
}

func (c *LocalClient) LookupIPv6(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP6)
}

func (c LocalClient) LookupDefault(ctx context.Context, domain string) ([]net.IP, error) {
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP)
}

func (c LocalClient) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	return transportInstance.Lookup(ctx, domain, strategy)
}

func (c LocalClient) QueryRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}
