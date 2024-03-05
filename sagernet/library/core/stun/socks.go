package stun

import (
	"net"

	"github.com/Dreamacro/clash/transport/socks5"
)

type socksPacketConn struct {
	*net.UDPConn
	tcpConn net.Conn
}

func (uc *socksPacketConn) WriteTo(b []byte, addr net.Addr) (n int, err error) {
	packet, err := socks5.EncodeUDPPacket(socks5.ParseAddrToSocksAddr(addr), b)
	if err != nil {
		return
	}
	return uc.UDPConn.Write(packet)
}

func (uc *socksPacketConn) ReadFrom(b []byte) (int, net.Addr, error) {
	_, _, err := uc.UDPConn.ReadFrom(b)
	if err != nil {
		return 0, nil, err
	}
	addr, payload, err := socks5.DecodeUDPPacket(b)
	if err != nil {
		return 0, nil, err
	}

	udpAddr := addr.UDPAddr()
	if udpAddr == nil {
		return 0, nil, newError("parse udp addr error")
	}

	// due to DecodeUDPPacket is mutable, record addr length
	copy(b, payload)
	return len(payload), udpAddr, nil
}

func (uc *socksPacketConn) Close() error {
	uc.tcpConn.Close()
	return uc.UDPConn.Close()
}
