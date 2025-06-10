package common

import (
	"strings"

	C "github.com/metacubex/mihomo/constant"
	"golang.org/x/net/idna"
)

type DomainSuffix struct {
	*Base
	suffix  string
	adapter string
}

func (ds *DomainSuffix) RuleType() C.RuleType {
	return C.DomainSuffix
}

func (ds *DomainSuffix) Match(metadata *C.Metadata, helper C.RuleMatchHelper) (bool, string) {
	domain := metadata.RuleHost()
	return strings.HasSuffix(domain, "."+ds.suffix) || domain == ds.suffix, ds.adapter
}

func (ds *DomainSuffix) Adapter() string {
	return ds.adapter
}

func (ds *DomainSuffix) Payload() string {
	return ds.suffix
}

func NewDomainSuffix(suffix string, adapter string) *DomainSuffix {
	punycode, _ := idna.ToASCII(strings.ToLower(suffix))
	return &DomainSuffix{
		Base:    &Base{},
		suffix:  punycode,
		adapter: adapter,
	}
}

//var _ C.Rule = (*DomainSuffix)(nil)
