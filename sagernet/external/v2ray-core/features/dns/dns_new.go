package dns

import (
	"context"
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
	LookupDefault(ctx context.Context, domain string) ([]net.IP, error)
	Lookup(ctx context.Context, domain string, strategy QueryStrategy) ([]net.IP, error)
	QueryRaw(ctx context.Context, message *buf.Buffer) (*buf.Buffer, error)
}

type Transport interface {
	SupportRaw() bool
	WriteMessage(ctx context.Context, message *dnsmessage.Message) error
	Lookup(ctx context.Context, domain string, strategy QueryStrategy) ([]net.IP, error)
}
