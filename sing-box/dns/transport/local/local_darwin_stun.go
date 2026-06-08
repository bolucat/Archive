//go:build darwin && !cgo

package local

import (
	"context"

	E "github.com/sagernet/sing/common/exceptions"

	mDNS "github.com/miekg/dns"
)

func newSystemResolver() systemResolver {
	return &cgoRequiredResolver{}
}

type cgoRequiredResolver struct{}

func (r *cgoRequiredResolver) Exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	return nil, E.New(`local DNS server requires CGO on darwin, rebuild with CGO_ENABLED=1`)
}

func (r *cgoRequiredResolver) Reset() {}

func (r *cgoRequiredResolver) Close() error {
	return nil
}
