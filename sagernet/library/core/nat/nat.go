package nat

import (
	"net"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/header/parse"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"libcore/comm"
	"libcore/tun"
)

//go:generate go run ../errorgen

var _ tun.Tun = (*SystemTun)(nil)

var (
	vlanClient4 = net.IPv4(172, 19, 0, 1)
	vlanClient6 = net.IP{0xfd, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x1}
)

type SystemTun struct {
	dispatcher   *readVDispatcher
	dev          int32
	mtu          int32
	handler      tun.Handler
	ipv6Mode     int32
	tcpForwarder *tcpForwarder
	errorHandler func(err string)
}

func New(dev int32, mtu int32, handler tun.Handler, ipv6Mode int32, errorHandler func(err string)) (*SystemTun, error) {
	t := &SystemTun{
		dev:          dev,
		mtu:          mtu,
		handler:      handler,
		ipv6Mode:     ipv6Mode,
		errorHandler: errorHandler,
	}
	dispatcher, err := newReadVDispatcher(int(dev), t)
	if err != nil {
		return nil, err
	}
	go dispatcher.dispatchLoop()
	t.dispatcher = dispatcher

	tcpServer, err := newTcpForwarder(t)
	if err != nil {
		return nil, err
	}
	go tcpServer.dispatchLoop()
	t.tcpForwarder = tcpServer

	return t, nil
}

func (n *SystemTun) deliverPacket(pkt *stack.PacketBuffer) {
	var ipVersion int
	if ihl, ok := pkt.Data().PullUp(1); ok {
		ipVersion = header.IPVersion(ihl)
	} else {
		return
	}

	log := "packet: "

	var ipHeader IPHeader
	switch ipVersion {
	case header.IPv4Version:
		if !parse.IPv4(pkt) {
			return
		}
		ipHeader = &IPv4Header{pkt, header.IPv4(pkt.NetworkHeader().View())}
		log += "ipv4: "
	case header.IPv6Version:
		proto, _, _, _, ok := parse.IPv6(pkt)
		if !ok {
			return
		}
		ipHeader = &IPv6Header{pkt, proto, header.IPv6(pkt.NetworkHeader().View())}
		log += "ipv6: "
	default:
		return
	}

	switch ipHeader.Protocol() {
	case header.TCPProtocolNumber:
		log += "tcp: "
		if !parse.TCP(pkt) {
			newError(log, "unable to parse").AtWarning().WriteToLog()
			return
		}
		if err := n.tcpForwarder.process(&TCPHeader{ipHeader, header.TCP(pkt.TransportHeader().View())}); err != nil {
			newError(log, "process failed").Base(err).AtWarning().WriteToLog()
			return
		}
		n.dispatcher.writePacket(pkt)
	case header.UDPProtocolNumber:
		log += "udp: "
		if !parse.UDP(pkt) {
			newError(log, "unable to parse").AtWarning().WriteToLog()
			return
		}
		n.processUDP(&UDPHeader{ipHeader, header.UDP(pkt.TransportHeader().View())})
	case header.ICMPv4ProtocolNumber:
		log += "icmp4: "
		if !parse.ICMPv4(pkt) {
			newError(log, "unable to parse").AtWarning().WriteToLog()
			return
		}
		n.processICMPv4(&ICMPv4Header{ipHeader.(*IPv4Header), header.ICMPv4(pkt.TransportHeader().View())})
	case header.ICMPv6ProtocolNumber:
		log += "icmp6: "
		if !parse.ICMPv6(pkt) {
			newError(log, "unable to parse").AtWarning().WriteToLog()
			return
		}
		n.processICMPv6(&ICMPv6Header{ipHeader.(*IPv6Header), header.ICMPv6(pkt.TransportHeader().View())})
	}
}

func (n *SystemTun) processUDP(hdr *UDPHeader) {
	sourceAddress := hdr.SourceAddress()
	destinationAddress := hdr.DestinationAddress()
	sourcePort := hdr.SourcePort()
	destinationPort := hdr.DestinationPort()

	source := v2rayNet.Destination{
		Address: v2rayNet.IPAddress([]byte(sourceAddress)),
		Port:    v2rayNet.Port(sourcePort),
		Network: v2rayNet.Network_UDP,
	}
	destination := v2rayNet.Destination{
		Address: v2rayNet.IPAddress([]byte(destinationAddress)),
		Port:    v2rayNet.Port(destinationPort),
		Network: v2rayNet.Network_UDP,
	}

	hdr.Packet().IncRef()
	hdr.SetDestinationAddress(sourceAddress)
	hdr.SetDestinationPort(sourcePort)

	data := hdr.Packet().Data().ExtractVV()
	go n.handler.NewPacket(source, destination, data.ToView(), func(bytes []byte, addr *v2rayNet.UDPAddr) (int, error) {
		buffer := buf.New()
		defer buffer.Release()

		var hdrLen int
		switch ipHdr := hdr.IPHeader.(type) {
		case *IPv4Header:
			hdrLen = int(ipHdr.IPv4.HeaderLength())
			buffer.Write(ipHdr.IPv4[:hdrLen])
		case *IPv6Header:
			hdrLen = len(ipHdr.IPv6) - int(ipHdr.IPv6.PayloadLength())
			buffer.Write(ipHdr.IPv6[:hdrLen])
		}
		buffer.Write(hdr.UDP[:header.UDPMinimumSize])
		buffer.Write(bytes)

		var newSourceAddress tcpip.Address
		var newSourcePort uint16

		if addr != nil {
			newSourceAddress = tcpip.Address(addr.IP)
			newSourcePort = uint16(addr.Port)
		} else {
			newSourceAddress = destinationAddress
			newSourcePort = destinationPort
		}

		switch hdr.IPHeader.(type) {
		case *IPv4Header:
			ipHdr := header.IPv4(buffer.Bytes())
			ipHdr.SetSourceAddress(newSourceAddress)
			ipHdr.SetTotalLength(uint16(buffer.Len()))
			ipHdr.SetChecksum(0)
			ipHdr.SetChecksum(^ipHdr.CalculateChecksum())
		case *IPv6Header:
			ipHdr := header.IPv6(buffer.Bytes())
			ipHdr.SetSourceAddress(newSourceAddress)
			ipHdr.SetPayloadLength(uint16(buffer.Len() - int32(hdrLen)))
		}

		udpHdr := header.UDP(buffer.BytesFrom(int32(hdrLen)))
		udpHdr.SetSourcePort(newSourcePort)
		udpHdr.SetLength(uint16(buffer.Len() - int32(hdrLen)))
		udpHdr.SetChecksum(0)
		udpHdr.SetChecksum(^udpHdr.CalculateChecksum(header.Checksum(bytes, header.PseudoHeaderChecksum(header.UDPProtocolNumber, newSourceAddress, sourceAddress, udpHdr.Length()))))

		if err := n.dispatcher.writeBuffer(buffer.Bytes()); err != nil {
			return 0, newError(err.String())
		}

		return len(bytes), nil
	}, comm.Closer(hdr.Packet().DecRef))
}

func (n *SystemTun) processICMPv4(hdr *ICMPv4Header) {
	if hdr.Type() != header.ICMPv4Echo || hdr.Code() != header.ICMPv4UnusedCode {
		return
	}

	source := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(hdr.SourceAddress())), Network: v2rayNet.Network_UDP}
	destination := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(hdr.DestinationAddress())), Port: 7, Network: v2rayNet.Network_UDP}

	sourceAddress := hdr.SourceAddress()
	hdr.SetSourceAddress(hdr.DestinationAddress())
	hdr.SetDestinationAddress(sourceAddress)
	hdr.IPv4Header.UpdateChecksum()

	dataVV := buffer.VectorisedView{}
	dataVV.AppendView(hdr.Packet().TransportHeader().View())
	dataVV.Append(hdr.Packet().Data().ExtractVV())
	data := dataVV.ToView()
	messageLen := len(data)

	netHdr := hdr.Packet().NetworkHeader().View()

	if n.handler.NewPingPacket(source, destination, data, func(message []byte) error {
		backData := buffer.VectorisedView{}
		backData.AppendView(netHdr)
		backData.AppendView(message)

		if len(message) != messageLen {
			ipHdr := header.IPv4(netHdr)
			oldLen := ipHdr.TotalLength()
			ipHdr.SetTotalLength(uint16(len(netHdr) + len(message)))
			ipHdr.SetChecksum(^header.ChecksumCombine(^ipHdr.Checksum(), header.ChecksumCombine(ipHdr.TotalLength(), ^oldLen)))
		}

		packet := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
		defer packet.DecRef()
		err := n.dispatcher.writePacket(packet)
		if err != nil {
			return newError("failed to write packet to device: ", err.String())
		}
		return nil
	}) {
		return
	}

	hdr.SetType(header.ICMPv4EchoReply)
	hdr.IPv4Header.UpdateChecksum()
	hdr.UpdateChecksum()
	n.dispatcher.writePacket(hdr.Packet())
}

func (n *SystemTun) processICMPv6(hdr *ICMPv6Header) {
	if hdr.Type() != header.ICMPv6EchoRequest {
		return
	}

	source := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(hdr.SourceAddress())), Network: v2rayNet.Network_UDP}
	destination := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(hdr.DestinationAddress())), Port: 7, Network: v2rayNet.Network_UDP}

	sourceAddress := hdr.SourceAddress()
	hdr.SetSourceAddress(hdr.DestinationAddress())
	hdr.SetDestinationAddress(sourceAddress)

	data := buffer.VectorisedView{}
	data.AppendView(hdr.Packet().TransportHeader().View())
	data.Append(hdr.Packet().Data().ExtractVV())

	netHdr := hdr.Packet().NetworkHeader().View()
	if n.handler.NewPingPacket(source, destination, data.ToView(), func(message []byte) error {
		backData := buffer.VectorisedView{}
		backData.AppendView(netHdr)
		backData.AppendView(message)

		icmpHdr := header.ICMPv6(message)
		icmpHdr.SetChecksum(0)
		icmpHdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
			Header: icmpHdr,
			Src:    hdr.SourceAddress(),
			Dst:    hdr.DestinationAddress(),
		}))

		packet := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
		defer packet.DecRef()
		err := n.dispatcher.writePacket(packet)
		if err != nil {
			return newError("failed to write packet to device: ", err.String())
		}
		return nil
	}) {
		return
	}

	hdr.SetType(header.ICMPv6EchoReply)
	hdr.UpdateChecksum()
	n.dispatcher.writePacket(hdr.Packet())
}

func (n *SystemTun) Close() error {
	n.dispatcher.stop()
	n.tcpForwarder.Close()
	return nil
}
