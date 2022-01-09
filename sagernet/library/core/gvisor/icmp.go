package gvisor

import (
	"github.com/v2fly/v2ray-core/v5/common/net"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"gvisor.dev/gvisor/pkg/tcpip/transport/icmp"
	"libcore/tun"
)

func gIcmpHandler(s *stack.Stack, ep stack.LinkEndpoint, handler tun.Handler) {
	s.SetTransportProtocolHandler(icmp.ProtocolNumber4, func(id stack.TransportEndpointID, packet *stack.PacketBuffer) bool {
		hdr := header.ICMPv4(packet.TransportHeader().View())
		if hdr.Type() != header.ICMPv4Echo {
			return false
		}

		source := net.Destination{Address: net.IPAddress([]byte(id.RemoteAddress)), Network: net.Network_UDP}
		destination := net.Destination{Address: net.IPAddress([]byte(id.LocalAddress)), Port: 7, Network: net.Network_UDP}

		ipHdr := header.IPv4(packet.NetworkHeader().View())
		sourceAddress := ipHdr.SourceAddress()
		ipHdr.SetSourceAddress(ipHdr.DestinationAddress())
		ipHdr.SetDestinationAddress(sourceAddress)
		ipHdr.SetChecksum(0)
		ipHdr.SetChecksum(^ipHdr.CalculateChecksum())

		dataVV := buffer.VectorisedView{}
		dataVV.AppendView(packet.TransportHeader().View())
		dataVV.Append(packet.Data().ExtractVV())
		data := dataVV.ToView()
		messageLen := len(data)

		netHdr := packet.NetworkHeader().View()
		if !handler.NewPingPacket(source, destination, data, func(message []byte) error {
			backData := buffer.VectorisedView{}
			backData.AppendView(netHdr)

			if len(message) != messageLen {
				backIpHdr := header.IPv4(backData.ToOwnedView())
				oldLen := backIpHdr.TotalLength()
				backIpHdr.SetTotalLength(uint16(len(netHdr) + len(message)))
				backIpHdr.SetChecksum(^header.ChecksumCombine(^backIpHdr.Checksum(), header.ChecksumCombine(backIpHdr.TotalLength(), ^oldLen)))
				backData = buffer.VectorisedView{}
				backData.AppendView(buffer.View(backIpHdr))
			}
			backData.AppendView(message)
			backPacket := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
			defer backPacket.DecRef()
			err := ep.WriteRawPacket(backPacket)
			if err != nil {
				return newError("failed to write packet to device: ", err.String())
			}
			return nil
		}) {
			hdr.SetType(header.ICMPv4EchoReply)
			hdr.SetChecksum(0)
			hdr.SetChecksum(header.ICMPv4Checksum(hdr, packet.Data().AsRange().Checksum()))
			err := ep.WriteRawPacket(packet)
			if err != nil {
				newError("failed to write packet to device: ", err.String()).AtWarning().WriteToLog()
				return false
			}
		}

		return true
	})
	s.SetTransportProtocolHandler(icmp.ProtocolNumber6, func(id stack.TransportEndpointID, packet *stack.PacketBuffer) bool {
		hdr := header.ICMPv6(packet.TransportHeader().View())
		if hdr.Type() != header.ICMPv6EchoRequest {
			return false
		}

		source := net.Destination{Address: net.IPAddress([]byte(id.RemoteAddress)), Network: net.Network_UDP}
		destination := net.Destination{Address: net.IPAddress([]byte(id.LocalAddress)), Port: 7, Network: net.Network_UDP}

		ipHdr := header.IPv6(packet.NetworkHeader().View())
		sourceAddress := ipHdr.SourceAddress()
		ipHdr.SetSourceAddress(ipHdr.DestinationAddress())
		ipHdr.SetDestinationAddress(sourceAddress)

		data := buffer.VectorisedView{}
		data.AppendView(packet.TransportHeader().View())
		data.Append(packet.Data().ExtractVV())

		netHdr := packet.NetworkHeader().View()
		if !handler.NewPingPacket(source, destination, data.ToView(), func(message []byte) error {
			backData := buffer.VectorisedView{}
			backData.AppendView(netHdr)
			backData.AppendView(message)

			icmpHdr := header.ICMPv6(message)
			icmpHdr.SetChecksum(0)
			icmpHdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
				Header: icmpHdr,
				Src:    id.LocalAddress,
				Dst:    id.RemoteAddress,
			}))

			backPacket := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
			defer backPacket.DecRef()
			err := ep.WriteRawPacket(backPacket)
			if err != nil {
				return newError("failed to write packet to device: ", err.String())
			}
			return nil
		}) {
			hdr.SetType(header.ICMPv6EchoReply)
			hdr.SetChecksum(0)
			hdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
				Header:      hdr,
				Src:         id.LocalAddress,
				Dst:         id.RemoteAddress,
				PayloadCsum: packet.Data().AsRange().Checksum(),
				PayloadLen:  packet.Data().Size(),
			}))
			err := ep.WriteRawPacket(packet)
			if err != nil {
				newError("failed to write packet to device: ", err.String()).AtWarning().WriteToLog()
				return false
			}
		}
		return true
	})
}
