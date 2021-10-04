package gvisor

import (
	"github.com/sirupsen/logrus"
	"github.com/xtls/xray-core/common/bytespool"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv4"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv6"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"gvisor.dev/gvisor/pkg/tcpip/transport/icmp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"
	"libcore/tun"
	"os"
)

var _ tun.Tun = (*GVisor)(nil)

type GVisor struct {
	Endpoint stack.LinkEndpoint
	Stack    *stack.Stack
}

func (t *GVisor) Close() error {
	t.Stack.Close()
	return nil
}

const DefaultNIC tcpip.NICID = 0x01

func New(dev *os.File, mtu int32, handler tun.Handler, nicId tcpip.NICID) (*GVisor, error) {
	endpoint := &rwEndpoint{rw: dev, mtu: uint32(mtu), pool: bytespool.GetPool(mtu)}
	s := stack.New(stack.Options{
		NetworkProtocols: []stack.NetworkProtocolFactory{
			ipv4.NewProtocol,
			ipv6.NewProtocol,
		},
		TransportProtocols: []stack.TransportProtocolFactory{
			tcp.NewProtocol,
			udp.NewProtocol,
			icmp.NewProtocol4,
			icmp.NewProtocol6,
		},
	})
	s.SetRouteTable([]tcpip.Route{
		{
			Destination: header.IPv4EmptySubnet,
			NIC:         nicId,
		},
		{
			Destination: header.IPv6EmptySubnet,
			NIC:         nicId,
		},
	})
	gTcpHandler(s, handler)
	gUdpHandler(s, handler)
	gMust(s.CreateNIC(nicId, endpoint))
	gMust(s.SetSpoofing(nicId, true))
	gMust(s.SetPromiscuousMode(nicId, true))

	return &GVisor{endpoint, s}, nil
}

func gMust(err tcpip.Error) {
	if err != nil {
		logrus.Panicln(err.String())
	}
}
