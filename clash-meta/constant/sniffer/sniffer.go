package sniffer

import "github.com/metacubex/mihomo/constant"

type Sniffer interface {
	SupportNetwork() constant.NetWork
	// SniffData must not change input bytes
	SniffData(bytes []byte) (string, error)
	Protocol() string
	SupportPort(port uint16) bool
}

type MultiPacketSniffer interface {
	WrapperSender(packetSender constant.PacketSender, override bool) constant.PacketSender
}

const (
	TLS Type = iota
	HTTP
	QUIC
)

var (
	List = []Type{TLS, HTTP, QUIC}
)

type Type int

func (rt Type) String() string {
	switch rt {
	case TLS:
		return "TLS"
	case HTTP:
		return "HTTP"
	case QUIC:
		return "QUIC"
	default:
		return "Unknown"
	}
}
