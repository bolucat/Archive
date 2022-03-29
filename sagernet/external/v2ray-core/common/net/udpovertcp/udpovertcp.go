package udpovertcp

import (
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/protocol"
)

//go:generate go run github.com/v2fly/v2ray-core/v5/common/errors/errorgen

const UOTMagicAddress = "sp.udp-over-tcp.arpa"

var addrParser = protocol.NewAddressParser(
	protocol.AddressFamilyByte(0x00, net.AddressFamilyIPv4),
	protocol.AddressFamilyByte(0x01, net.AddressFamilyIPv6),
	protocol.AddressFamilyByte(0x02, net.AddressFamilyDomain),
)

func GetDestinationSubsetOf(dest net.Destination) bool {
	return dest.Address.Family().IsDomain() && dest.Address.Domain() == UOTMagicAddress
}
