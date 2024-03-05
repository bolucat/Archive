package libcore

import (
	"os"

	"github.com/sagernet/libping"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"libcore/stun"
)

//go:generate go run ./errorgen

func Setenv(key, value string) error {
	return os.Setenv(key, value)
}

func Unsetenv(key string) error {
	return os.Unsetenv(key)
}

func IcmpPing(address string, timeout int32) (int32, error) {
	return libping.IcmpPing(address, timeout)
}

const (
	StunNoResult int32 = iota
	StunEndpointIndependentNoNAT
	StunEndpointIndependent
	StunAddressDependent
	StunAddressAndPortDependent
)

type StunResult struct {
	NatMapping   int32
	NatFiltering int32
}

func StunTest(serverAddress string, socksPort int32) (*StunResult, error) {
	natMapping, natFiltering, err := stun.Test(serverAddress, int(socksPort))
	if err != nil {
		return nil, err
	}
	return &StunResult{
		NatMapping:   int32(natMapping),
		NatFiltering: int32(natFiltering),
	}, nil
}

func EnableConnectionPool() {
	net.EnableConnectionPool()
}

func DisableConnectionPool() {
	net.DisableConnectionPool()
}

func ResetConnections() {
	net.ResetConnections()
}
