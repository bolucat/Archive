package outbound

import (
	"context"
	"errors"
	"github.com/metacubex/mihomo/component/dialer"
	"github.com/metacubex/mihomo/component/loopback"
	"github.com/metacubex/mihomo/component/resolver"
	C "github.com/metacubex/mihomo/constant"
)

type Direct struct {
	*Base
	loopBack *loopback.Detector
}

type DirectOption struct {
	BasicOption
	Name string `proxy:"name"`
}

// DialContext implements C.ProxyAdapter
func (d *Direct) DialContext(ctx context.Context, metadata *C.Metadata) (C.Conn, error) {
	if err := d.loopBack.CheckConn(metadata); err != nil {
		return nil, err
	}
	opts := d.DialOptions()
	opts = append(opts, dialer.WithResolver(resolver.DirectHostResolver))
	c, err := dialer.DialContext(ctx, "tcp", metadata.RemoteAddress(), opts...)
	if err != nil {
		return nil, err
	}
	return d.loopBack.NewConn(NewConn(c, d)), nil
}

// ListenPacketContext implements C.ProxyAdapter
func (d *Direct) ListenPacketContext(ctx context.Context, metadata *C.Metadata) (C.PacketConn, error) {
	if err := d.loopBack.CheckPacketConn(metadata); err != nil {
		return nil, err
	}
	// net.UDPConn.WriteTo only working with *net.UDPAddr, so we need a net.UDPAddr
	if !metadata.Resolved() {
		ip, err := resolver.ResolveIPWithResolver(ctx, metadata.Host, resolver.DirectHostResolver)
		if err != nil {
			return nil, errors.New("can't resolve ip")
		}
		metadata.DstIP = ip
	}
	pc, err := dialer.NewDialer(d.DialOptions()...).ListenPacket(ctx, "udp", "", metadata.AddrPort())
	if err != nil {
		return nil, err
	}
	return d.loopBack.NewPacketConn(newPacketConn(pc, d)), nil
}

func (d *Direct) IsL3Protocol(metadata *C.Metadata) bool {
	return true // tell DNSDialer don't send domain to DialContext, avoid lookback to DefaultResolver
}

func NewDirectWithOption(option DirectOption) *Direct {
	return &Direct{
		Base: &Base{
			name:   option.Name,
			tp:     C.Direct,
			udp:    true,
			tfo:    option.TFO,
			mpTcp:  option.MPTCP,
			iface:  option.Interface,
			rmark:  option.RoutingMark,
			prefer: C.NewDNSPrefer(option.IPVersion),
		},
		loopBack: loopback.NewDetector(),
	}
}

func NewDirect() *Direct {
	return &Direct{
		Base: &Base{
			name:   "DIRECT",
			tp:     C.Direct,
			udp:    true,
			prefer: C.DualStack,
		},
		loopBack: loopback.NewDetector(),
	}
}

func NewCompatible() *Direct {
	return &Direct{
		Base: &Base{
			name:   "COMPATIBLE",
			tp:     C.Compatible,
			udp:    true,
			prefer: C.DualStack,
		},
		loopBack: loopback.NewDetector(),
	}
}
