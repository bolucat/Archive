package inbound

import (
	"errors"
	"fmt"
	"strings"

	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/listener/socks"
	"github.com/metacubex/mihomo/log"
)

type SocksOption struct {
	BaseOption
	Users AuthUsers `inbound:"users,omitempty"`
	UDP   bool      `inbound:"udp,omitempty"`
}

func (o SocksOption) Equal(config C.InboundConfig) bool {
	return optionToString(o) == optionToString(config)
}

type Socks struct {
	*Base
	config *SocksOption
	udp    bool
	stl    []*socks.Listener
	sul    []*socks.UDPListener
}

func NewSocks(options *SocksOption) (*Socks, error) {
	base, err := NewBase(&options.BaseOption)
	if err != nil {
		return nil, err
	}
	return &Socks{
		Base:   base,
		config: options,
		udp:    options.UDP,
	}, nil
}

// Config implements constant.InboundListener
func (s *Socks) Config() C.InboundConfig {
	return s.config
}

// Close implements constant.InboundListener
func (s *Socks) Close() error {
	var errs []error
	for _, l := range s.stl {
		err := l.Close()
		if err != nil {
			errs = append(errs, fmt.Errorf("close tcp listener %s err: %w", l.Address(), err))
		}
	}
	for _, l := range s.sul {
		err := l.Close()
		if err != nil {
			errs = append(errs, fmt.Errorf("close udp listener %s err: %w", l.Address(), err))
		}
	}
	if len(errs) > 0 {
		return errors.Join(errs...)
	}
	return nil
}

// Address implements constant.InboundListener
func (s *Socks) Address() string {
	var addrList []string
	for _, l := range s.stl {
		addrList = append(addrList, l.Address())
	}
	return strings.Join(addrList, ",")
}

// Listen implements constant.InboundListener
func (s *Socks) Listen(tunnel C.Tunnel) error {
	for _, addr := range strings.Split(s.RawAddress(), ",") {
		stl, err := socks.NewWithAuthenticator(addr, tunnel, s.config.Users.GetAuthStore(), s.Additions()...)
		if err != nil {
			return err
		}
		s.stl = append(s.stl, stl)
		if s.udp {
			sul, err := socks.NewUDP(addr, tunnel, s.Additions()...)
			if err != nil {
				return err
			}
			s.sul = append(s.sul, sul)
		}
	}

	log.Infoln("SOCKS[%s] proxy listening at: %s", s.Name(), s.Address())
	return nil
}

var _ C.InboundListener = (*Socks)(nil)
