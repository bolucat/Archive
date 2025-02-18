package inbound

import (
	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/listener/anytls"
	LC "github.com/metacubex/mihomo/listener/config"
	"github.com/metacubex/mihomo/log"
)

type AnyTLSOption struct {
	BaseOption
	Users         map[string]string `inbound:"users,omitempty"`
	Certificate   string            `inbound:"certificate"`
	PrivateKey    string            `inbound:"private-key"`
	PaddingScheme string            `inbound:"padding-scheme,omitempty"`
}

func (o AnyTLSOption) Equal(config C.InboundConfig) bool {
	return optionToString(o) == optionToString(config)
}

type AnyTLS struct {
	*Base
	config *AnyTLSOption
	l      C.MultiAddrListener
	vs     LC.AnyTLSServer
}

func NewAnyTLS(options *AnyTLSOption) (*AnyTLS, error) {
	base, err := NewBase(&options.BaseOption)
	if err != nil {
		return nil, err
	}
	return &AnyTLS{
		Base:   base,
		config: options,
		vs: LC.AnyTLSServer{
			Enable:        true,
			Listen:        base.RawAddress(),
			Users:         options.Users,
			Certificate:   options.Certificate,
			PrivateKey:    options.PrivateKey,
			PaddingScheme: options.PaddingScheme,
		},
	}, nil
}

// Config implements constant.InboundListener
func (v *AnyTLS) Config() C.InboundConfig {
	return v.config
}

// Address implements constant.InboundListener
func (v *AnyTLS) Address() string {
	if v.l != nil {
		for _, addr := range v.l.AddrList() {
			return addr.String()
		}
	}
	return ""
}

// Listen implements constant.InboundListener
func (v *AnyTLS) Listen(tunnel C.Tunnel) error {
	var err error
	v.l, err = anytls.New(v.vs, tunnel, v.Additions()...)
	if err != nil {
		return err
	}
	log.Infoln("AnyTLS[%s] proxy listening at: %s", v.Name(), v.Address())
	return nil
}

// Close implements constant.InboundListener
func (v *AnyTLS) Close() error {
	return v.l.Close()
}

var _ C.InboundListener = (*AnyTLS)(nil)
