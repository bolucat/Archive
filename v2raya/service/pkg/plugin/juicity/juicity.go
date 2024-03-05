package juicity

import (
	"fmt"
	"net"

	"github.com/daeuniverse/outbound/dialer"
	"github.com/daeuniverse/outbound/dialer/juicity"
	"github.com/daeuniverse/softwind/netproxy"
	"github.com/v2rayA/v2rayA/pkg/plugin"

	_ "github.com/daeuniverse/softwind/protocol/juicity"
)

// Juicity is a base juicity struct
type Juicity struct {
	dialer netproxy.Dialer
}

func init() {
	plugin.RegisterDialer("juicity", NewJuicityDialer)
}

func NewJuicityDialer(s string, d plugin.Dialer) (plugin.Dialer, error) {

	dialer, _, err := juicity.NewJuicity(
		&dialer.ExtraOption{},
		&plugin.Converter{
			Dialer: d,
		},
		s,
	)
	if err != nil {
		return nil, err
	}
	return &Juicity{
		dialer: dialer,
	}, nil
}

// Addr returns forwarder's address.
func (s *Juicity) Addr() string {
	return ""
}

// Dial connects to the address addr on the network net via the infra.
func (s *Juicity) Dial(network, addr string) (net.Conn, error) {
	return s.dial(network, addr)
}

func (s *Juicity) dial(network, addr string) (net.Conn, error) {
	rc, err := s.dialer.Dial("tcp", addr)
	if err != nil {
		return nil, fmt.Errorf("[juicity]: dial to %s: %w", addr, err)
	}
	return &netproxy.FakeNetConn{
		Conn:  rc,
		LAddr: nil,
		RAddr: nil,
	}, err
}

// DialUDP connects to the given address via the infra.
func (s *Juicity) DialUDP(network, addr string) (net.PacketConn, net.Addr, error) {
	rc, err := s.dialer.Dial("udp", addr)
	if err != nil {
		return nil, nil, fmt.Errorf("[juicity]: dial to %s: %w", addr, err)
	}
	return &netproxy.FakeNetPacketConn{
		PacketConn: rc.(netproxy.PacketConn),
		LAddr:      nil,
		RAddr:      nil,
	}, nil, err
}
