package inbound_test

import (
	"net/netip"
	"testing"

	"github.com/metacubex/mihomo/adapter/outbound"
	"github.com/metacubex/mihomo/listener/inbound"

	"github.com/stretchr/testify/assert"
)

func testInboundAnyTLS(t *testing.T, inboundOptions inbound.AnyTLSOption, outboundOptions outbound.AnyTLSOption) {
	t.Parallel()
	inboundOptions.BaseOption = inbound.BaseOption{
		NameStr: "anytls_inbound",
		Listen:  "127.0.0.1",
		Port:    "0",
	}
	inboundOptions.Users = map[string]string{"test": userUUID}
	in, err := inbound.NewAnyTLS(&inboundOptions)
	if !assert.NoError(t, err) {
		return
	}

	tunnel := NewHttpTestTunnel()
	defer tunnel.Close()

	err = in.Listen(tunnel)
	if !assert.NoError(t, err) {
		return
	}
	defer in.Close()

	addrPort, err := netip.ParseAddrPort(in.Address())
	if !assert.NoError(t, err) {
		return
	}

	outboundOptions.Name = "anytls_outbound"
	outboundOptions.Server = addrPort.Addr().String()
	outboundOptions.Port = int(addrPort.Port())
	outboundOptions.Password = userUUID

	out, err := outbound.NewAnyTLS(outboundOptions)
	if !assert.NoError(t, err) {
		return
	}
	defer out.Close()

	tunnel.DoTest(t, out)
}

func TestInboundAnyTLS_TLS(t *testing.T) {
	inboundOptions := inbound.AnyTLSOption{
		Certificate: tlsCertificate,
		PrivateKey:  tlsPrivateKey,
	}
	outboundOptions := outbound.AnyTLSOption{
		Fingerprint: tlsFingerprint,
	}
	testInboundAnyTLS(t, inboundOptions, outboundOptions)
}
