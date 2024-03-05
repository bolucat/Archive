package wireguard

import (
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/buffer"
	"gvisor.dev/gvisor/pkg/tcpip/header"
	"gvisor.dev/gvisor/pkg/tcpip/stack"
)

var _ stack.LinkEndpoint = (*wireEndpoint)(nil)

type wireEndpoint struct {
	*wireDevice
}

func (w *wireEndpoint) AddHeader(*stack.PacketBuffer) {
}

func (w *wireEndpoint) MTU() uint32 {
	return uint32(w.mtu)
}

func (w *wireEndpoint) MaxHeaderLength() uint16 {
	return 0
}

func (w *wireEndpoint) LinkAddress() tcpip.LinkAddress {
	return ""
}

func (w *wireEndpoint) Capabilities() stack.LinkEndpointCapabilities {
	return stack.CapabilityNone
}

func (w *wireEndpoint) Attach(dispatcher stack.NetworkDispatcher) {
	w.dispatcher = dispatcher
}

func (w *wireEndpoint) IsAttached() bool {
	return w.dispatcher != nil
}

func (w *wireEndpoint) Wait() {
}

func (w *wireEndpoint) ARPHardwareType() header.ARPHardwareType {
	return header.ARPHardwareNone
}

func (w *wireEndpoint) WritePackets(pkts stack.PacketBufferList) (int, tcpip.Error) {
	w.access.Lock()
	defer w.access.Unlock()
	if w.done.Done() {
		return 0, &tcpip.ErrClosedForSend{}
	}
	n := 0
	for pkt := pkts.Front(); pkt != nil; pkt = pkt.Next() {
		w.outbound <- buffer.NewVectorisedView(pkt.Size(), pkt.Views())
		n++
	}
	return n, nil
}

func (w *wireEndpoint) WriteRawPacket(packet *stack.PacketBuffer) tcpip.Error {
	w.access.Lock()
	defer w.access.Unlock()
	if w.done.Done() {
		return &tcpip.ErrClosedForSend{}
	}
	w.outbound <- packet.Data().ExtractVV()
	return nil
}
