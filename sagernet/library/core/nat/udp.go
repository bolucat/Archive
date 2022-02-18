package nat

import (
	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"libcore/comm"
)

func (t *SystemTun) processIPv4UDP(ipHdr header.IPv4, hdr header.UDP) {
	cache := buf.New()

	sourceAddress := ipHdr.SourceAddress()
	destinationAddress := ipHdr.DestinationAddress()
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

	ipHdr.SetDestinationAddress(sourceAddress)
	hdr.SetDestinationPort(sourcePort)

	headerLength := ipHdr.HeaderLength()

	headerCache := cache.ExtendCopy(ipHdr[:headerLength+header.UDPMinimumSize])
	cache.Advance(cache.Len())
	cache.Write(hdr.Payload())

	go t.handler.NewPacket(source, destination, cache.Bytes(), func(bytes []byte, addr *v2rayNet.UDPAddr) (int, error) {
		replyCache := buf.New()
		defer replyCache.Release()
		replyCache.Write(headerCache)

		var newSourceAddress tcpip.Address
		var newSourcePort uint16

		if addr != nil {
			newSourceAddress = tcpip.Address(addr.IP)
			newSourcePort = uint16(addr.Port)
		} else {
			newSourceAddress = destinationAddress
			newSourcePort = destinationPort
		}

		newIpHdr := header.IPv4(replyCache.Bytes())
		newIpHdr.SetSourceAddress(newSourceAddress)
		newIpHdr.SetTotalLength(uint16(int(replyCache.Len()) + len(bytes)))
		newIpHdr.SetChecksum(0)
		newIpHdr.SetChecksum(^newIpHdr.CalculateChecksum())

		udpHdr := header.UDP(replyCache.BytesFrom(replyCache.Len() - header.UDPMinimumSize))
		udpHdr.SetSourcePort(newSourcePort)
		udpHdr.SetLength(uint16(header.UDPMinimumSize + len(bytes)))
		udpHdr.SetChecksum(0)
		udpHdr.SetChecksum(^udpHdr.CalculateChecksum(header.Checksum(bytes, header.PseudoHeaderChecksum(header.UDPProtocolNumber, newSourceAddress, sourceAddress, uint16(header.UDPMinimumSize+len(bytes))))))

		replyVV := buffer.VectorisedView{}
		replyVV.AppendView(replyCache.Bytes())
		replyVV.AppendView(bytes)

		if err := t.writeRawPacket(replyVV); err != nil {
			return 0, newError(err.String())
		}

		return len(bytes), nil
	}, comm.Closer(cache.Release))
}

func (t *SystemTun) processIPv6UDP(ipHdr header.IPv6, hdr header.UDP) {
	cache := buf.New()

	sourceAddress := ipHdr.SourceAddress()
	destinationAddress := ipHdr.DestinationAddress()
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

	ipHdr.SetDestinationAddress(sourceAddress)
	hdr.SetDestinationPort(sourcePort)

	headerLength := uint16(len(ipHdr)) - ipHdr.PayloadLength()

	headerCache := cache.ExtendCopy(ipHdr[:headerLength+header.UDPMinimumSize])
	cache.Advance(cache.Len())
	cache.Write(hdr.Payload())

	go t.handler.NewPacket(source, destination, cache.Bytes(), func(bytes []byte, addr *v2rayNet.UDPAddr) (int, error) {
		replyCache := buf.New()
		defer replyCache.Release()
		replyCache.Write(headerCache)

		var newSourceAddress tcpip.Address
		var newSourcePort uint16

		if addr != nil {
			newSourceAddress = tcpip.Address(addr.IP)
			newSourcePort = uint16(addr.Port)
		} else {
			newSourceAddress = destinationAddress
			newSourcePort = destinationPort
		}

		newIpHdr := header.IPv6(replyCache.Bytes())
		newIpHdr.SetSourceAddress(newSourceAddress)
		newIpHdr.SetPayloadLength(uint16(header.UDPMinimumSize + len(bytes)))

		udpHdr := header.UDP(replyCache.BytesFrom(replyCache.Len() - header.UDPMinimumSize))
		udpHdr.SetSourcePort(newSourcePort)
		udpHdr.SetLength(uint16(header.UDPMinimumSize + len(bytes)))
		udpHdr.SetChecksum(0)
		udpHdr.SetChecksum(^udpHdr.CalculateChecksum(header.Checksum(bytes, header.PseudoHeaderChecksum(header.UDPProtocolNumber, newSourceAddress, sourceAddress, uint16(header.UDPMinimumSize+len(bytes))))))

		replyVV := buffer.VectorisedView{}
		replyVV.AppendView(replyCache.Bytes())
		replyVV.AppendView(bytes)

		if err := t.writeRawPacket(replyVV); err != nil {
			return 0, newError(err.String())
		}

		return len(bytes), nil
	}, comm.Closer(cache.Release))
}
