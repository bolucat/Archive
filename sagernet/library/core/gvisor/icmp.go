package gvisor

import (
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"golang.org/x/sys/unix"
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

		originVV := buffer.VectorisedView{}
		originVV.AppendView(packet.NetworkHeader().View())
		transportData := packet.TransportHeader().View()
		if transportData.Size() > 8 {
			transportData = transportData[:8]
		}
		originVV.AppendView(transportData)
		originHdr := originVV.ToOwnedView()

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
		if !handler.NewPingPacket(source, destination, buf.FromBytes(data), func(message []byte) error {
			icmpHdr := header.ICMPv4(message)
			if icmpHdr.Type() == header.ICMPv4DstUnreachable {
				const ICMPv4HeaderSize = 4
				unreachableHdr := header.ICMPv4(buffer.NewView(header.ICMPv4MinimumErrorPayloadSize + len(originHdr)))
				copy(unreachableHdr[:ICMPv4HeaderSize], message)
				copy(unreachableHdr[header.ICMPv4MinimumErrorPayloadSize:], originHdr)
				icmpHdr = unreachableHdr
			}

			backData := buffer.VectorisedView{}

			if len(icmpHdr) != messageLen {
				backIpHdr := header.IPv4(buffer.NewViewFromBytes(netHdr))
				oldLen := backIpHdr.TotalLength()
				backIpHdr.SetTotalLength(uint16(len(netHdr) + len(message)))
				backIpHdr.SetChecksum(^header.ChecksumCombine(^backIpHdr.Checksum(), header.ChecksumCombine(backIpHdr.TotalLength(), ^oldLen)))
				backData.AppendView(buffer.View(backIpHdr))
			} else {
				backData.AppendView(netHdr)
			}

			backData.AppendView(buffer.View(icmpHdr))
			backPacket := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
			defer backPacket.DecRef()
			var packetList stack.PacketBufferList
			packetList.PushFront(backPacket)
			_, err := ep.WritePackets(packetList)
			if err != nil {
				return newError("failed to write packet to device: ", err.String())
			}

			if icmpHdr.Type() == header.ICMPv4DstUnreachable {
				return unix.ENETUNREACH
			}

			return nil
		}, nil) {
			hdr.SetType(header.ICMPv4EchoReply)
			hdr.SetChecksum(0)
			hdr.SetChecksum(header.ICMPv4Checksum(hdr, packet.Data().AsRange().Checksum()))
			var packetList stack.PacketBufferList
			packetList.PushFront(packet)
			_, err := ep.WritePackets(packetList)
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

		originVV := buffer.VectorisedView{}
		originVV.AppendView(packet.NetworkHeader().View())
		transportData := packet.TransportHeader().View()
		if transportData.Size() > 8 {
			transportData = transportData[:8]
		}
		originVV.AppendView(transportData)
		originHdr := originVV.ToOwnedView()

		ipHdr := header.IPv6(packet.NetworkHeader().View())
		sourceAddress := ipHdr.SourceAddress()
		ipHdr.SetSourceAddress(ipHdr.DestinationAddress())
		ipHdr.SetDestinationAddress(sourceAddress)

		dataVV := buffer.VectorisedView{}
		dataVV.AppendView(packet.TransportHeader().View())
		dataVV.Append(packet.Data().ExtractVV())
		data := dataVV.ToView()
		messageLen := len(data)

		netHdr := packet.NetworkHeader().View()
		if !handler.NewPingPacket(source, destination, buf.FromBytes(data), func(message []byte) error {
			icmpHdr := header.ICMPv6(message)
			if icmpHdr.Type() == header.ICMPv6DstUnreachable {
				unreachableHdr := header.ICMPv6(buffer.NewView(header.ICMPv6DstUnreachableMinimumSize + len(originHdr)))
				copy(unreachableHdr[:header.ICMPv6HeaderSize], message)
				copy(unreachableHdr[header.ICMPv6DstUnreachableMinimumSize:], originHdr)
				icmpHdr = unreachableHdr
			}

			backData := buffer.VectorisedView{}

			if len(icmpHdr) != messageLen {
				backIpHdr := header.IPv6(buffer.NewViewFromBytes(netHdr))
				backIpHdr.SetPayloadLength(uint16(len(icmpHdr)))
				backData.AppendView(buffer.View(backIpHdr))
			} else {
				backData.AppendView(netHdr)
			}

			backData.AppendView(buffer.View(icmpHdr))

			icmpHdr.SetChecksum(0)
			icmpHdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
				Header: icmpHdr,
				Src:    id.RemoteAddress,
				Dst:    id.LocalAddress,
			}))

			backPacket := stack.NewPacketBuffer(stack.PacketBufferOptions{Data: backData})
			defer backPacket.DecRef()
			var packetList stack.PacketBufferList
			packetList.PushFront(backPacket)
			_, err := ep.WritePackets(packetList)
			if err != nil {
				return newError("failed to write packet to device: ", err.String())
			}

			if icmpHdr.Type() == header.ICMPv6DstUnreachable {
				return unix.ENETUNREACH
			}

			return nil
		}, nil) {
			hdr.SetType(header.ICMPv6EchoReply)
			hdr.SetChecksum(0)
			hdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
				Header:      hdr,
				Src:         id.LocalAddress,
				Dst:         id.RemoteAddress,
				PayloadCsum: packet.Data().AsRange().Checksum(),
				PayloadLen:  packet.Data().Size(),
			}))
			var packetList stack.PacketBufferList
			packetList.PushFront(packet)
			_, err := ep.WritePackets(packetList)
			if err != nil {
				newError("failed to write packet to device: ", err.String()).AtWarning().WriteToLog()
				return false
			}
		}
		return true
	})
}
