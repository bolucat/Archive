package localdns

import (
	"context"
	"github.com/v2fly/v2ray-core/v4/common/net"
	"github.com/v2fly/v2ray-core/v4/features/dns"
)

// Client is an implementation of dns.Client, which queries localhost for DNS.
type Client struct {
	resolver *net.Resolver
}

// Type implements common.HasType.
func (*Client) Type() interface{} {
	return dns.ClientType()
}

// Start implements common.Runnable.
func (*Client) Start() error {
	return nil
}

// Close implements common.Closable.
func (*Client) Close() error { return nil }

// LookupIP implements Client.
func (c *Client) LookupIP(host string) ([]net.IP, error) {
	return c.resolver.LookupIP(context.Background(), "ip", host)
}

// LookupIPv4 implements IPv4Lookup.
func (c *Client) LookupIPv4(host string) ([]net.IP, error) {
	return c.resolver.LookupIP(context.Background(), "ip4", host)
}

// LookupIPv6 implements IPv6Lookup.
func (c *Client) LookupIPv6(host string) ([]net.IP, error) {
	return c.resolver.LookupIP(context.Background(), "ip6", host)
}

// New create a new dns.Client that queries localhost for DNS.
func New() *Client {
	return &Client{
		resolver: &net.Resolver{
			PreferGo: false,
		},
	}
}
