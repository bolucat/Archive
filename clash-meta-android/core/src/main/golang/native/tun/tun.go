package tun

import (
	"io"
	"net/netip"

	C "github.com/metacubex/mihomo/constant"
	LC "github.com/metacubex/mihomo/listener/config"
	"github.com/metacubex/mihomo/listener/sing_tun"
	"github.com/metacubex/mihomo/log"
	"github.com/metacubex/mihomo/tunnel"
)

func Start(fd int, gateway, portal, dns string) (io.Closer, error) {
	log.Debugln("TUN: fd = %d, gateway = %s, portal = %s, dns = %s", fd, gateway, portal, dns)

	options := LC.Tun{
		Enable:         true,
		Device:         sing_tun.InterfaceName,
		Stack:          C.TunSystem,
		DNSHijack:      []string{dns + ":53"},                          // "172.19.0.2" or "0.0.0.0"
		Inet4Address:   []netip.Prefix{netip.MustParsePrefix(gateway)}, // "172.19.0.1/30"
		MTU:            9000,                                           // private const val TUN_MTU = 9000 in TunService.kt
		FileDescriptor: fd,
	}

	listener, err := sing_tun.New(options, tunnel.Tunnel)
	if err != nil {
		log.Errorln("TUN:", err)
		return nil, err
	}

	return listener, nil
}
