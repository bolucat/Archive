package nat

import (
	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"golang.org/x/sys/unix"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"libcore/comm"
)

func (t *SystemTun) processICMPv4(ipHdr header.IPv4, hdr header.ICMPv4) {
	if hdr.Type() != header.ICMPv4Echo || hdr.Code() != header.ICMPv4UnusedCode {
		return
	}

	source := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(ipHdr.SourceAddress())), Network: v2rayNet.Network_UDP}
	destination := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(ipHdr.DestinationAddress())), Port: 7, Network: v2rayNet.Network_UDP}

	sourceAddress := ipHdr.SourceAddress()
	ipHdr.SetSourceAddress(ipHdr.DestinationAddress())
	ipHdr.SetDestinationAddress(sourceAddress)
	ipHdr.SetChecksum(0)
	ipHdr.SetChecksum(^ipHdr.CalculateChecksum())

	cache := buf.New()

	netHdr := cache.ExtendCopy(ipHdr[:ipHdr.HeaderLength()])
	transportDataLen := len(hdr)
	if transportDataLen > 8 {
		transportDataLen = 8
	}
	cache.ExtendCopy(hdr[:transportDataLen])
	originHdr := cache.Bytes()
	cache.Advance(cache.Len())

	cache.Write(hdr)
	messageLen := len(hdr)

	if t.handler.NewPingPacket(source, destination, cache.Bytes(), func(message []byte) error {
		replyCache := buf.New()
		defer replyCache.Release()

		icmpHdr := header.ICMPv4(message)
		if icmpHdr.Type() == header.ICMPv4DstUnreachable {
			const ICMPv4HeaderSize = 4
			unreachableHdr := header.ICMPv4(replyCache.Extend(int32(header.ICMPv4MinimumErrorPayloadSize + len(originHdr))))
			copy(unreachableHdr[:ICMPv4HeaderSize], message)
			copy(unreachableHdr[header.ICMPv4MinimumErrorPayloadSize:], originHdr)
			icmpHdr = unreachableHdr
		}

		backData := buffer.VectorisedView{}

		if len(icmpHdr) != messageLen {
			backIpHdr := header.IPv4(replyCache.ExtendCopy(netHdr))
			oldLen := backIpHdr.TotalLength()
			backIpHdr.SetTotalLength(uint16(len(netHdr) + len(message)))
			backIpHdr.SetChecksum(^header.ChecksumCombine(^backIpHdr.Checksum(), header.ChecksumCombine(backIpHdr.TotalLength(), ^oldLen)))
			backData.AppendView(buffer.View(backIpHdr))
		} else {
			backData.AppendView(netHdr)
		}

		backData.AppendView(buffer.View(icmpHdr))
		err := t.writeRawPacket(backData)
		if err != nil {
			return newError("failed to write packet to device: ", err.String())
		}
		if icmpHdr.Type() == header.ICMPv4DstUnreachable {
			return unix.ENETUNREACH
		}
		return nil
	}, comm.Closer(cache.Release)) {
		return
	}

	hdr.SetType(header.ICMPv4EchoReply)
	hdr.SetChecksum(0)
	hdr.SetChecksum(header.ICMPv4Checksum(hdr, 0))

	t.writeBuffer(ipHdr)
}

func (t *SystemTun) processICMPv6(ipHdr header.IPv6, hdr header.ICMPv6) {
	if hdr.Type() != header.ICMPv6EchoRequest || hdr.Code() != header.ICMPv6UnusedCode {
		return
	}

	source := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(ipHdr.SourceAddress())), Network: v2rayNet.Network_UDP}
	destination := v2rayNet.Destination{Address: v2rayNet.IPAddress([]byte(ipHdr.DestinationAddress())), Port: 7, Network: v2rayNet.Network_UDP}

	sourceAddress := ipHdr.SourceAddress()
	ipHdr.SetSourceAddress(ipHdr.DestinationAddress())
	ipHdr.SetDestinationAddress(sourceAddress)

	cache := buf.New()

	netHdr := cache.ExtendCopy(ipHdr[:len(ipHdr)-int(ipHdr.PayloadLength())])
	transportDataLen := len(hdr)
	if transportDataLen > 8 {
		transportDataLen = 8
	}
	cache.ExtendCopy(hdr[:transportDataLen])
	originHdr := cache.Bytes()
	cache.Advance(cache.Len())

	cache.Write(hdr)
	messageLen := len(hdr)

	if t.handler.NewPingPacket(source, destination, cache.Bytes(), func(message []byte) error {
		replyCache := buf.New()
		defer replyCache.Release()

		icmpHdr := header.ICMPv6(message)
		if icmpHdr.Type() == header.ICMPv6DstUnreachable {
			unreachableHdr := header.ICMPv6(replyCache.Extend(int32(header.ICMPv6DstUnreachableMinimumSize + len(originHdr))))
			copy(unreachableHdr[:header.ICMPv6HeaderSize], message)
			copy(unreachableHdr[header.ICMPv6DstUnreachableMinimumSize:], originHdr)
			icmpHdr = unreachableHdr
		}

		backData := buffer.VectorisedView{}

		if len(icmpHdr) != messageLen {
			backIpHdr := header.IPv6(replyCache.ExtendCopy(netHdr))
			backIpHdr.SetPayloadLength(uint16(len(icmpHdr)))
			backData.AppendView(buffer.View(backIpHdr))
		} else {
			backData.AppendView(netHdr)
		}

		icmpHdr.SetChecksum(0)
		icmpHdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
			Header: icmpHdr,
			Src:    ipHdr.SourceAddress(),
			Dst:    ipHdr.DestinationAddress(),
		}))

		backData.AppendView(buffer.View(icmpHdr))
		err := t.writeRawPacket(backData)
		if err != nil {
			return newError("failed to write packet to device: ", err.String())
		}
		if icmpHdr.Type() == header.ICMPv6DstUnreachable {
			return unix.ENETUNREACH
		}
		return nil
	}, comm.Closer(cache.Release)) {
		return
	}

	hdr.SetType(header.ICMPv6EchoReply)
	hdr.SetChecksum(0)
	hdr.SetChecksum(header.ICMPv6Checksum(header.ICMPv6ChecksumParams{
		Header: hdr,
		Src:    ipHdr.SourceAddress(),
		Dst:    ipHdr.DestinationAddress(),
	}))

	t.writeBuffer(ipHdr)
}
