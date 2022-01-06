package localdns

import (
	"context"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

var (
	Instance   = &Client{}
	LookupFunc func(network string, host string) ([]net.IP, error)
)

func init() {
	SetLookupFunc(nil)
}

func SetLookupFunc(lookupFunc func(network, host string) ([]net.IP, error)) {
	if lookupFunc == nil {
		resolver := &net.Resolver{PreferGo: false}
		LookupFunc = func(network string, host string) ([]net.IP, error) {
			return resolver.LookupIP(context.Background(), network, host)
		}
	} else {
		LookupFunc = lookupFunc
	}
}

// Client is an implementation of dns.Client, which queries localhost for DNS.
type Client struct {
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
	return LookupFunc("ip", host)
}

// LookupIPv4 implements IPv4Lookup.
func (c *Client) LookupIPv4(host string) ([]net.IP, error) {
	return LookupFunc("ip4", host)
}

// LookupIPv6 implements IPv6Lookup.
func (c *Client) LookupIPv6(host string) ([]net.IP, error) {
	return LookupFunc("ip6", host)
}

// New create a new dns.Client that queries localhost for DNS.
func New() *Client {
	return Instance
}
