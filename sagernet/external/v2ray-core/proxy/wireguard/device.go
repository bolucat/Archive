package wireguard

import (
	"io"
	"os"
	"sync"

	"golang.zx2c4.com/wireguard/tun"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/header/parse"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv4"
	"gvisor.dev/gvisor/pkg/tcpip/network/ipv6"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"gvisor.dev/gvisor/pkg/tcpip/transport/icmp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/tcp"
	"gvisor.dev/gvisor/pkg/tcpip/transport/udp"

	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/pingproto"
	"github.com/v2fly/v2ray-core/v5/common/signal/done"
)

var _ tun.Device = (*wireDevice)(nil)

const defaultNIC tcpip.NICID = 1

type wireDevice struct {
	stack       *stack.Stack
	mtu         int
	events      chan tun.Event
	outbound    chan buffer.VectorisedView
	dispatcher  stack.NetworkDispatcher
	access      sync.Mutex
	icmpManager *pingproto.ClientManager
	done        *done.Instance
	addr4       tcpip.Address
	addr6       tcpip.Address
}

func newDevice(localAddresses []net.IP, mtu int, icmpManager *pingproto.ClientManager) (device *wireDevice, err error) {
	opts := stack.Options{
		NetworkProtocols:   []stack.NetworkProtocolFactory{ipv4.NewProtocol, ipv6.NewProtocol},
		TransportProtocols: []stack.TransportProtocolFactory{tcp.NewProtocol, udp.NewProtocol, icmp.NewProtocol4, icmp.NewProtocol6},
		HandleLocal:        true,
	}
	s := stack.New(opts)
	device = &wireDevice{
		stack:       s,
		mtu:         mtu,
		events:      make(chan tun.Event, 4),
		outbound:    make(chan buffer.VectorisedView, 256),
		icmpManager: icmpManager,
		done:        done.New(),
	}
	if err := s.CreateNIC(defaultNIC, &wireEndpoint{device}); err != nil {
		return nil, newError("failed to create gVisor nic :" + err.String())
	}
	for _, ip := range localAddresses {
		var protoAddr tcpip.ProtocolAddress
		if ip4 := ip.To4(); ip4 != nil {
			addr := tcpip.Address(ip4)
			device.addr4 = addr
			protoAddr = tcpip.ProtocolAddress{
				Protocol:          ipv4.ProtocolNumber,
				AddressWithPrefix: addr.WithPrefix(),
			}
		} else {
			addr := tcpip.Address(ip)
			device.addr6 = addr
			protoAddr = tcpip.ProtocolAddress{
				Protocol:          ipv6.ProtocolNumber,
				AddressWithPrefix: addr.WithPrefix(),
			}
		}
		if err := s.AddProtocolAddress(defaultNIC, protoAddr, stack.AddressProperties{}); err != nil {
			return nil, newError("failed to AddProtocolAddress ", protoAddr.AddressWithPrefix.Address, ": ", err)
		}
	}

	s.AddRoute(tcpip.Route{Destination: header.IPv4EmptySubnet, NIC: defaultNIC})
	s.AddRoute(tcpip.Route{Destination: header.IPv6EmptySubnet, NIC: defaultNIC})
	device.events <- tun.EventUp

	return
}

func (w *wireDevice) File() *os.File {
	return nil
}

func (w *wireDevice) Read(buf []byte, offset int) (int, error) {
	packet, ok := <-w.outbound
	if !ok {
		return 0, os.ErrClosed
	}
	return packet.Read(buf[offset:])
}

func (w *wireDevice) writePing4(message []byte, addr net.Addr) (int, error) {
	remote := tcpip.Address(net.DestinationFromAddr(addr).Address.IP())
	route, err := w.stack.FindRoute(defaultNIC, w.addr4, remote, header.IPv4ProtocolNumber, false)
	if err != nil {
		return 0, newError("failed to find route for ", remote, ": ", err)
	}
	defer route.Release()

	hdr := header.ICMPv4(message)
	hdr.SetChecksum(0)
	hdr.SetChecksum(header.ICMPv4Checksum(hdr, 0))

	pkt := stack.NewPacketBuffer(stack.PacketBufferOptions{
		ReserveHeaderBytes: int(route.MaxHeaderLength()),
		Data:               buffer.View(message).ToVectorisedView(),
	})
	defer pkt.DecRef()

	err = route.WritePacket(stack.NetworkHeaderParams{
		Protocol: header.ICMPv4ProtocolNumber,
		TTL:      route.DefaultTTL(),
	}, pkt)
	if err != nil {
		return 0, newError("failed to write ping to wireguard: ", err)
	}
	return len(message), nil
}

func (w *wireDevice) writePing6(message []byte, addr net.Addr) (int, error) {
	remote := tcpip.Address(net.DestinationFromAddr(addr).Address.IP())
	route, err := w.stack.FindRoute(defaultNIC, w.addr6, remote, header.IPv6ProtocolNumber, false)
	if err != nil {
		return 0, newError("failed to find route for ", remote, ": ", err)
	}
	defer route.Release()

	hdr := header.ICMPv6(message)
	hdr.SetChecksum(0)
	hdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
		Header: hdr,
		Src:    w.addr6,
		Dst:    remote,
	}))

	pkt := stack.NewPacketBuffer(stack.PacketBufferOptions{
		ReserveHeaderBytes: int(route.MaxHeaderLength()),
		Data:               buffer.View(hdr).ToVectorisedView(),
	})
	defer pkt.DecRef()
	err = route.WritePacket(stack.NetworkHeaderParams{
		Protocol: header.ICMPv6ProtocolNumber,
		TTL:      route.DefaultTTL(),
	}, pkt)
	if err != nil {
		return 0, newError("failed to write ping6 to wireguard: ", err)
	}
	return len(message), nil
}

func (w *wireDevice) Write(buf []byte, offset int) (int, error) {
	if w.done.Done() {
		return 0, io.ErrClosedPipe
	}

	packet := buf[offset:]
	if len(packet) == 0 {
		return 0, nil
	}

	var (
		networkProtocol   tcpip.NetworkProtocolNumber
		transportProtocol tcpip.TransportProtocolNumber
	)

	pkb := stack.NewPacketBuffer(stack.PacketBufferOptions{
		Data: buffer.View(packet).ToVectorisedView(),
	})

	for {
		switch header.IPVersion(packet) {
		case header.IPv4Version:
			if !parse.IPv4(pkb) {
				break
			}
			transportProtocol = pkb.Network().TransportProtocol()
		case header.IPv6Version:
			proto, _, _, _, ok := parse.IPv6(pkb)
			if !ok {
				break
			}
			transportProtocol = proto
		}
		networkProtocol = pkb.NetworkProtocolNumber
		if transportProtocol == header.ICMPv4ProtocolNumber {
			data := pkb.Data().ExtractVV()
			message := data.ToView()
			hdr := header.ICMPv4(message)
			if hdr.Type() != header.ICMPv4EchoReply {
				break
			}
			newError("icmp recv ", hdr.Sequence(), " from ", pkb.Network().SourceAddress()).AtDebug().WriteToLog()
			client := w.icmpManager.GetClient(net.Destination{Address: net.IPAddress([]byte(pkb.Network().SourceAddress()))})
			client.WriteBack4(message)
			return len(packet), nil
		} else if transportProtocol == header.ICMPv6ProtocolNumber {
			data := pkb.Data().ExtractVV()
			message := data.ToView()
			hdr := header.ICMPv6(message)
			if hdr.Type() != header.ICMPv6EchoReply {
				break
			}
			newError("icmp6 recv ", hdr.Sequence(), " from ", pkb.Network().SourceAddress()).AtDebug().WriteToLog()
			client := w.icmpManager.GetClient(net.Destination{Address: net.IPAddress([]byte(pkb.Network().SourceAddress()))})
			client.WriteBack6(message)
			return len(packet), nil
		}
		break
	}
	pkb.DecRef()
	pkb = stack.NewPacketBuffer(stack.PacketBufferOptions{
		Data: buffer.NewViewFromBytes(packet).ToVectorisedView(),
	})
	w.dispatcher.DeliverNetworkPacket("", "", networkProtocol, pkb)
	pkb.DecRef()
	return len(packet), nil
}

func (w *wireDevice) Flush() error {
	return nil
}

func (w *wireDevice) MTU() (int, error) {
	return w.mtu, nil
}

func (w *wireDevice) Name() (string, error) {
	return "v2ray", nil
}

func (w *wireDevice) Events() chan tun.Event {
	return w.events
}

func (w *wireDevice) Close() error {
	w.access.Lock()
	defer w.access.Unlock()
	w.done.Close()
	w.stack.Close()
	close(w.outbound)
	return nil
}
