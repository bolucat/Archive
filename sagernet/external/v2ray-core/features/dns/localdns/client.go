package localdns

import (
	"context"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

var _ dns.NewClient = (*Client)(nil)

var instance dns.NewClient = &Client{}

type Client struct{}

func NewClient() dns.NewClient {
	return instance
}

func (c Client) Type() interface{} {
	return dns.ClientType()
}

func (c Client) Start() error {
	return nil
}

func (c Client) Close() error {
	return nil
}

func (c *Client) LookupIP(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.LookupDefault(ctx, domain)
}

func (c *Client) LookupIPv4(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP4)
}

func (c *Client) LookupIPv6(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(context.Background(), dns.DefaultTimeout)
	defer cancel()
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP6)
}

func (c Client) LookupDefault(ctx context.Context, domain string) ([]net.IP, error) {
	return c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP)
}

func (c Client) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	return transportInstance.Lookup(ctx, domain, strategy)
}

func (c Client) QueryRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}
