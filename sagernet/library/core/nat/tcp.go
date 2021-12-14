package nat

import (
	"net"
	"sync"

	v2rayNet "github.com/v2fly/v2ray-core/v4/common/net"
	"gvisor.dev/gvisor/pkg/tcpip"
)

type tcpForwarder struct {
	tun      *SystemTun
	port     uint16
	listener *net.TCPListener
	sessions sync.Map
}

type tcpSession struct {
	sourceAddress      tcpip.Address
	destinationAddress tcpip.Address
	sourcePort         uint16
	destinationPort    uint16
}

func newTcpForwarder(tun *SystemTun) (*tcpForwarder, error) {
	var network string
	address := &net.TCPAddr{}
	if tun.ipv6Mode == 0 {
		network = "tcp4"
		address.IP = vlanClient4
	} else {
		network = "tcp"
		address.IP = net.IPv6zero
	}
	listener, err := net.ListenTCP(network, address)
	if err != nil {
		return nil, newError("failed to create tcp forwarder at ", address.IP).Base(err)
	}
	addr := listener.Addr().(*net.TCPAddr)
	port := uint16(addr.Port)
	newError("tcp forwarder started at ", addr).AtDebug().WriteToLog()
	return &tcpForwarder{tun, port, listener, sync.Map{}}, nil
}

func (t *tcpForwarder) dispatch() (bool, error) {
	conn, err := t.listener.AcceptTCP()
	if err != nil {
		return true, err
	}
	newError("accepted tcp connection: ", conn.RemoteAddr()).AtDebug().WriteToLog()
	addr := conn.RemoteAddr().(*net.TCPAddr)
	sourcePort := uint16(addr.Port)

	var session *tcpSession
	iSession, ok := t.sessions.Load(sourcePort)
	if ok {
		session = iSession.(*tcpSession)
	} else {
		conn.Close()
		return false, newError("dropped unknown tcp session with source port ", sourcePort)
	}

	source := v2rayNet.Destination{
		Address: v2rayNet.IPAddress([]byte(session.sourceAddress)),
		Port:    v2rayNet.Port(session.sourcePort),
		Network: v2rayNet.Network_TCP,
	}
	destination := v2rayNet.Destination{
		Address: v2rayNet.IPAddress([]byte(session.destinationAddress)),
		Port:    v2rayNet.Port(session.destinationPort),
		Network: v2rayNet.Network_TCP,
	}

	newError("create connection: ", source, " => ", destination).AtDebug().WriteToLog()

	go func() {
		t.tun.handler.NewConnection(source, destination, conn)
		t.sessions.Delete(sourcePort)
	}()
	return false, newError()
}

func (t *tcpForwarder) dispatchLoop() {
	for {
		stop, err := t.dispatch()
		if err != nil {
			e := newError("dispatch tcp conn failed").Base(err)
			e.WriteToLog()
			if stop {
				t.Close()
				t.tun.errorHandler(e.String())
				return
			}
		}
	}
}

func (t *tcpForwarder) process(hdr *TCPHeader) error {
	sourceAddress := hdr.SourceAddress()
	destinationAddress := hdr.DestinationAddress()
	sourcePort := hdr.SourcePort()
	destinationPort := hdr.DestinationPort()

	var session *tcpSession

	if sourcePort != t.port {
		iSession, ok := t.sessions.Load(sourcePort)
		if ok {
			session = iSession.(*tcpSession)
		} else {
			/*if hdr.Flags() != header.TCPFlagSyn {
				return newError("unable to create session: not tcp syn flag")
			}*/
			session = &tcpSession{sourceAddress, destinationAddress, sourcePort, destinationPort}
			t.sessions.Store(sourcePort, session)
		}

		hdr.SetSourceAddress(destinationAddress)
		hdr.SetDestinationAddress(hdr.Device())
		hdr.SetDestinationPort(t.port)
		hdr.UpdateChecksum()

		// destinationAddress:sourcePort -> device:tcpServerPort
	} else {
		// device:tcpServerPort -> destinationAddress:sourcePort

		iSession, ok := t.sessions.Load(destinationPort)
		if ok {
			session = iSession.(*tcpSession)
		} else {
			return newError("unknown tcp session with source port ", destinationPort)
		}
		hdr.SetSourceAddress(destinationAddress)
		hdr.SetSourcePort(session.destinationPort)
		hdr.SetDestinationAddress(session.sourceAddress)
		hdr.UpdateChecksum()
	}

	return nil
}

func (t *tcpForwarder) Close() error {
	return t.listener.Close()
}
