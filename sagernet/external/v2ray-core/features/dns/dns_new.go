package dns

import (
	"context"
	"io"
	"time"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
)

const DefaultTimeout = time.Second * 5

type NewClient interface {
	Client
	IPv4Lookup
	IPv6Lookup
	LookupDefault(ctx context.Context, domain string) ([]net.IP, uint32, error)
	Lookup(ctx context.Context, domain string, strategy QueryStrategy) ([]net.IP, uint32, error)
	QueryRaw(ctx context.Context, message *buf.Buffer) (*buf.Buffer, error)
}

type TransportType uint8

const (
	TransportTypeDefault TransportType = iota
	TransportTypeExchange
	TransportTypeExchangeRaw
	TransportTypeLookup
)

type Transport interface {
	Type() TransportType
	Write(ctx context.Context, message *dnsmessage.Message) error
	Exchange(ctx context.Context, message *dnsmessage.Message) (*dnsmessage.Message, error)
	ExchangeRaw(ctx context.Context, message *buf.Buffer) (*buf.Buffer, error)
	Lookup(ctx context.Context, domain string, strategy QueryStrategy) ([]net.IP, error)
	io.Closer
}
