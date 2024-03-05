package nat

import (
	"os"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"golang.org/x/sys/unix"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/header/parse"
	"gvisor.dev/gvisor/pkg/tcpip/link/rawfile"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
	"libcore/tun"
)

//go:generate go run ../errorgen

var _ tun.Tun = (*SystemTun)(nil)

var (
	vlanClient4 = tcpip.Address([]uint8{172, 19, 0, 1})
	vlanClient6 = tcpip.Address([]uint8{0xfd, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x1})
)

type SystemTun struct {
	dev          int
	mtu          int
	handler      tun.Handler
	ipv6Mode     int32
	tcpForwarder *tcpForwarder
	errorHandler func(err string)
}

func New(dev int32, mtu int32, handler tun.Handler, ipv6Mode int32, errorHandler func(err string)) (*SystemTun, error) {
	t := &SystemTun{
		dev:          int(dev),
		mtu:          int(mtu),
		handler:      handler,
		ipv6Mode:     ipv6Mode,
		errorHandler: errorHandler,
	}
	tcpServer, err := newTcpForwarder(t)
	if err != nil {
		return nil, err
	}
	go tcpServer.dispatchLoop()
	t.tcpForwarder = tcpServer

	go t.dispatchLoop()
	return t, nil
}

func (t *SystemTun) dispatchLoop() {
	cache := buf.NewSize(int32(t.mtu))
	defer cache.Release()
	data := cache.Use()

	device := os.NewFile(uintptr(t.dev), "tun")
	element := v2rayNet.AddConnection(device)
	defer v2rayNet.RemoveConnection(element)

	for {
		n, err := device.Read(data)
		if err != nil {
			break
		}
		cache.Clear()
		cache.Resize(0, int32(n))
		packet := data[:n]
		if t.deliverPacket(cache, packet) {
			cache = buf.New()
			data = cache.Extend(buf.Size)
		}
	}
}

func (t *SystemTun) writeRawPacket(vv buffer.VectorisedView) tcpip.Error {
	views := vv.Views()
	iovecs := make([]unix.Iovec, len(views))
	for i, v := range views {
		iovecs[i] = rawfile.IovecFromBytes(v)
	}
	return rawfile.NonBlockingWriteIovec(t.dev, iovecs)
}

func (t *SystemTun) writeBuffer(bytes []byte) tcpip.Error {
	return rawfile.NonBlockingWrite(t.dev, bytes)
}

func (t *SystemTun) deliverPacket(cache *buf.Buffer, packet []byte) bool {
	switch header.IPVersion(packet) {
	case header.IPv4Version:
		ipHdr := header.IPv4(packet)
		switch ipHdr.TransportProtocol() {
		case header.TCPProtocolNumber:
			t.tcpForwarder.processIPv4(ipHdr, ipHdr.Payload())
		case header.UDPProtocolNumber:
			t.processIPv4UDP(cache, ipHdr, ipHdr.Payload())
			return true
		case header.ICMPv4ProtocolNumber:
			return t.processICMPv4(cache, ipHdr, ipHdr.Payload())
		}
	case header.IPv6Version:
		pkt := stack.NewPacketBuffer(stack.PacketBufferOptions{
			Data: buffer.View(packet).ToVectorisedView(),
		})
		proto, _, _, _, ok := parse.IPv6(pkt)
		pkt.DecRef()
		if !ok {
			return false
		}
		ipHdr := header.IPv6(packet)
		switch proto {
		case header.TCPProtocolNumber:
			t.tcpForwarder.processIPv6(ipHdr, ipHdr.Payload())
		case header.UDPProtocolNumber:
			t.processIPv6UDP(cache, ipHdr, ipHdr.Payload())
			return true
		case header.ICMPv6ProtocolNumber:
			return t.processICMPv6(cache, ipHdr, ipHdr.Payload())
		}
	}
	return false
}

func (t *SystemTun) Close() error {
	return t.tcpForwarder.Close()
}
