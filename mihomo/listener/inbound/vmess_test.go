package inbound_test

import (
	"net"
	"net/netip"
	"testing"

	"github.com/metacubex/mihomo/adapter/outbound"
	"github.com/metacubex/mihomo/common/utils"
	"github.com/metacubex/mihomo/listener/inbound"
	"github.com/stretchr/testify/assert"
)

func testInboundVMess(t *testing.T, inboundOptions inbound.VmessOption, outboundOptions outbound.VmessOption) {
	userUUID := utils.NewUUIDV4().String()
	inboundOptions.BaseOption = inbound.BaseOption{
		NameStr: "vmess_inbound",
		Listen:  "127.0.0.1",
		Port:    "0",
	}
	inboundOptions.Users = []inbound.VmessUser{
		{Username: "test", UUID: userUUID, AlterID: 0},
	}
	in, err := inbound.NewVmess(&inboundOptions)
	assert.NoError(t, err)

	tunnel := NewHttpTestTunnel()
	defer tunnel.Close()

	err = in.Listen(tunnel)
	assert.NoError(t, err)
	defer in.Close()

	addrPort, err := netip.ParseAddrPort(in.Address())
	assert.NoError(t, err)

	outboundOptions.Name = "vmess_outbound"
	outboundOptions.Server = addrPort.Addr().String()
	outboundOptions.Port = int(addrPort.Port())
	outboundOptions.UUID = userUUID
	outboundOptions.AlterID = 0
	outboundOptions.Cipher = "auto"

	out, err := outbound.NewVmess(outboundOptions)
	assert.NoError(t, err)
	defer out.Close()

	tunnel.DoTest(t, out)
}

func TestInboundVMess_Basic(t *testing.T) {
	inboundOptions := inbound.VmessOption{}
	outboundOptions := outbound.VmessOption{}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Tls(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		Certificate: tlsCertificate,
		PrivateKey:  tlsPrivateKey,
	}
	outboundOptions := outbound.VmessOption{
		TLS:         true,
		Fingerprint: tlsFingerprint,
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Ws(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		WsPath: "/ws",
	}
	outboundOptions := outbound.VmessOption{
		Network: "ws",
		WSOpts: outbound.WSOptions{
			Path: "/ws",
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Ws_ed1(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		WsPath: "/ws",
	}
	outboundOptions := outbound.VmessOption{
		Network: "ws",
		WSOpts: outbound.WSOptions{
			Path: "/ws?ed=2048",
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Ws_ed2(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		WsPath: "/ws",
	}
	outboundOptions := outbound.VmessOption{
		Network: "ws",
		WSOpts: outbound.WSOptions{
			Path:                "/ws",
			MaxEarlyData:        2048,
			EarlyDataHeaderName: "Sec-WebSocket-Protocol",
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Ws_Upgrade1(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		WsPath: "/ws",
	}
	outboundOptions := outbound.VmessOption{
		Network: "ws",
		WSOpts: outbound.WSOptions{
			Path:             "/ws",
			V2rayHttpUpgrade: true,
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Ws_Upgrade2(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		WsPath: "/ws",
	}
	outboundOptions := outbound.VmessOption{
		Network: "ws",
		WSOpts: outbound.WSOptions{
			Path:                     "/ws",
			V2rayHttpUpgrade:         true,
			V2rayHttpUpgradeFastOpen: true,
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Wss1(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		Certificate: tlsCertificate,
		PrivateKey:  tlsPrivateKey,
		WsPath:      "/ws",
	}
	outboundOptions := outbound.VmessOption{
		TLS:         true,
		Fingerprint: tlsFingerprint,
		Network:     "ws",
		WSOpts: outbound.WSOptions{
			Path: "/ws",
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Wss2(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		Certificate:     tlsCertificate,
		PrivateKey:      tlsPrivateKey,
		WsPath:          "/ws",
		GrpcServiceName: "GunService",
	}
	outboundOptions := outbound.VmessOption{
		TLS:         true,
		Fingerprint: tlsFingerprint,
		Network:     "ws",
		WSOpts: outbound.WSOptions{
			Path: "/ws",
		},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Grpc1(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		Certificate:     tlsCertificate,
		PrivateKey:      tlsPrivateKey,
		GrpcServiceName: "GunService",
	}
	outboundOptions := outbound.VmessOption{
		TLS:         true,
		Fingerprint: tlsFingerprint,
		Network:     "grpc",
		GrpcOpts:    outbound.GrpcOptions{GrpcServiceName: "GunService"},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Grpc2(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		Certificate:     tlsCertificate,
		PrivateKey:      tlsPrivateKey,
		WsPath:          "/ws",
		GrpcServiceName: "GunService",
	}
	outboundOptions := outbound.VmessOption{
		TLS:         true,
		Fingerprint: tlsFingerprint,
		Network:     "grpc",
		GrpcOpts:    outbound.GrpcOptions{GrpcServiceName: "GunService"},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Reality(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		RealityConfig: inbound.RealityConfig{
			Dest:        net.JoinHostPort(realityDest, "443"),
			PrivateKey:  realityPrivateKey,
			ShortID:     []string{realityShortid},
			ServerNames: []string{realityDest},
		},
	}
	outboundOptions := outbound.VmessOption{
		TLS:        true,
		ServerName: realityDest,
		RealityOpts: outbound.RealityOptions{
			PublicKey: realityPublickey,
			ShortID:   realityShortid,
		},
		ClientFingerprint: "chrome",
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}

func TestInboundVMess_Reality_Grpc(t *testing.T) {
	inboundOptions := inbound.VmessOption{
		RealityConfig: inbound.RealityConfig{
			Dest:        net.JoinHostPort(realityDest, "443"),
			PrivateKey:  realityPrivateKey,
			ShortID:     []string{realityShortid},
			ServerNames: []string{realityDest},
		},
		GrpcServiceName: "GunService",
	}
	outboundOptions := outbound.VmessOption{
		TLS:        true,
		ServerName: realityDest,
		RealityOpts: outbound.RealityOptions{
			PublicKey: realityPublickey,
			ShortID:   realityShortid,
		},
		ClientFingerprint: "chrome",
		Network:           "grpc",
		GrpcOpts:          outbound.GrpcOptions{GrpcServiceName: "GunService"},
	}
	testInboundVMess(t, inboundOptions, outboundOptions)
}
