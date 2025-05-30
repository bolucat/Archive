package outbound

import (
	"context"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"net"
	"net/netip"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/metacubex/mihomo/common/atomic"
	"github.com/metacubex/mihomo/component/dialer"
	"github.com/metacubex/mihomo/component/proxydialer"
	"github.com/metacubex/mihomo/component/resolver"
	"github.com/metacubex/mihomo/component/slowdown"
	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/dns"
	"github.com/metacubex/mihomo/log"

	amnezia "github.com/metacubex/amneziawg-go/device"
	wireguard "github.com/metacubex/sing-wireguard"
	"github.com/metacubex/wireguard-go/device"

	"github.com/metacubex/sing/common/debug"
	E "github.com/metacubex/sing/common/exceptions"
	M "github.com/metacubex/sing/common/metadata"
)

type wireguardGoDevice interface {
	Close()
	IpcSet(uapiConf string) error
}

type WireGuard struct {
	*Base
	bind      *wireguard.ClientBind
	device    wireguardGoDevice
	tunDevice wireguard.Device
	dialer    proxydialer.SingDialer
	resolver  resolver.Resolver

	initOk        atomic.Bool
	initMutex     sync.Mutex
	initErr       error
	option        WireGuardOption
	connectAddr   M.Socksaddr
	localPrefixes []netip.Prefix

	serverAddrMap   map[M.Socksaddr]netip.AddrPort
	serverAddrTime  atomic.TypedValue[time.Time]
	serverAddrMutex sync.Mutex
}

type WireGuardOption struct {
	BasicOption
	WireGuardPeerOption
	Name                string `proxy:"name"`
	Ip                  string `proxy:"ip,omitempty"`
	Ipv6                string `proxy:"ipv6,omitempty"`
	PrivateKey          string `proxy:"private-key"`
	Workers             int    `proxy:"workers,omitempty"`
	MTU                 int    `proxy:"mtu,omitempty"`
	UDP                 bool   `proxy:"udp,omitempty"`
	PersistentKeepalive int    `proxy:"persistent-keepalive,omitempty"`

	AmneziaWGOption *AmneziaWGOption `proxy:"amnezia-wg-option,omitempty"`

	Peers []WireGuardPeerOption `proxy:"peers,omitempty"`

	RemoteDnsResolve bool     `proxy:"remote-dns-resolve,omitempty"`
	Dns              []string `proxy:"dns,omitempty"`

	RefreshServerIPInterval int `proxy:"refresh-server-ip-interval,omitempty"`
}

type WireGuardPeerOption struct {
	Server       string   `proxy:"server"`
	Port         int      `proxy:"port"`
	PublicKey    string   `proxy:"public-key,omitempty"`
	PreSharedKey string   `proxy:"pre-shared-key,omitempty"`
	Reserved     []uint8  `proxy:"reserved,omitempty"`
	AllowedIPs   []string `proxy:"allowed-ips,omitempty"`
}

type AmneziaWGOption struct {
	JC   int    `proxy:"jc"`
	JMin int    `proxy:"jmin"`
	JMax int    `proxy:"jmax"`
	S1   int    `proxy:"s1"`
	S2   int    `proxy:"s2"`
	H1   uint32 `proxy:"h1"`
	H2   uint32 `proxy:"h2"`
	H3   uint32 `proxy:"h3"`
	H4   uint32 `proxy:"h4"`
}

type wgSingErrorHandler struct {
	name string
}

var _ E.Handler = (*wgSingErrorHandler)(nil)

func (w wgSingErrorHandler) NewError(ctx context.Context, err error) {
	if E.IsClosedOrCanceled(err) {
		log.SingLogger.Debug(fmt.Sprintf("[WG](%s) connection closed: %s", w.name, err))
		return
	}
	log.SingLogger.Error(fmt.Sprintf("[WG](%s) %s", w.name, err))
}

type wgNetDialer struct {
	tunDevice wireguard.Device
}

var _ dialer.NetDialer = (*wgNetDialer)(nil)

func (d wgNetDialer) DialContext(ctx context.Context, network, address string) (net.Conn, error) {
	return d.tunDevice.DialContext(ctx, network, M.ParseSocksaddr(address).Unwrap())
}

func (option WireGuardPeerOption) Addr() M.Socksaddr {
	return M.ParseSocksaddrHostPort(option.Server, uint16(option.Port))
}

func (option WireGuardOption) Prefixes() ([]netip.Prefix, error) {
	localPrefixes := make([]netip.Prefix, 0, 2)
	if len(option.Ip) > 0 {
		if !strings.Contains(option.Ip, "/") {
			option.Ip = option.Ip + "/32"
		}
		if prefix, err := netip.ParsePrefix(option.Ip); err == nil {
			localPrefixes = append(localPrefixes, prefix)
		} else {
			return nil, E.Cause(err, "ip address parse error")
		}
	}
	if len(option.Ipv6) > 0 {
		if !strings.Contains(option.Ipv6, "/") {
			option.Ipv6 = option.Ipv6 + "/128"
		}
		if prefix, err := netip.ParsePrefix(option.Ipv6); err == nil {
			localPrefixes = append(localPrefixes, prefix)
		} else {
			return nil, E.Cause(err, "ipv6 address parse error")
		}
	}
	if len(localPrefixes) == 0 {
		return nil, E.New("missing local address")
	}
	return localPrefixes, nil
}

func NewWireGuard(option WireGuardOption) (*WireGuard, error) {
	outbound := &WireGuard{
		Base: &Base{
			name:   option.Name,
			addr:   net.JoinHostPort(option.Server, strconv.Itoa(option.Port)),
			tp:     C.WireGuard,
			udp:    option.UDP,
			iface:  option.Interface,
			rmark:  option.RoutingMark,
			prefer: C.NewDNSPrefer(option.IPVersion),
		},
	}
	singDialer := proxydialer.NewSlowDownSingDialer(proxydialer.NewByNameSingDialer(option.DialerProxy, dialer.NewDialer(outbound.DialOptions()...)), slowdown.New())
	outbound.dialer = singDialer

	var reserved [3]uint8
	if len(option.Reserved) > 0 {
		if len(option.Reserved) != 3 {
			return nil, E.New("invalid reserved value, required 3 bytes, got ", len(option.Reserved))
		}
		copy(reserved[:], option.Reserved)
	}
	var isConnect bool
	if len(option.Peers) < 2 {
		isConnect = true
		if len(option.Peers) == 1 {
			outbound.connectAddr = option.Peers[0].Addr()
		} else {
			outbound.connectAddr = option.Addr()
		}
	}
	outbound.bind = wireguard.NewClientBind(context.Background(), wgSingErrorHandler{outbound.Name()}, outbound.dialer, isConnect, outbound.connectAddr.AddrPort(), reserved)

	var err error
	outbound.localPrefixes, err = option.Prefixes()
	if err != nil {
		return nil, err
	}

	{
		bytes, err := base64.StdEncoding.DecodeString(option.PrivateKey)
		if err != nil {
			return nil, E.Cause(err, "decode private key")
		}
		option.PrivateKey = hex.EncodeToString(bytes)
	}

	if len(option.Peers) > 0 {
		for i := range option.Peers {
			peer := &option.Peers[i] // we need modify option here
			bytes, err := base64.StdEncoding.DecodeString(peer.PublicKey)
			if err != nil {
				return nil, E.Cause(err, "decode public key for peer ", i)
			}
			peer.PublicKey = hex.EncodeToString(bytes)

			if peer.PreSharedKey != "" {
				bytes, err := base64.StdEncoding.DecodeString(peer.PreSharedKey)
				if err != nil {
					return nil, E.Cause(err, "decode pre shared key for peer ", i)
				}
				peer.PreSharedKey = hex.EncodeToString(bytes)
			}

			if len(peer.AllowedIPs) == 0 {
				return nil, E.New("missing allowed_ips for peer ", i)
			}

			if len(peer.Reserved) > 0 {
				if len(peer.Reserved) != 3 {
					return nil, E.New("invalid reserved value for peer ", i, ", required 3 bytes, got ", len(peer.Reserved))
				}
			}
		}
	} else {
		{
			bytes, err := base64.StdEncoding.DecodeString(option.PublicKey)
			if err != nil {
				return nil, E.Cause(err, "decode peer public key")
			}
			option.PublicKey = hex.EncodeToString(bytes)
		}
		if option.PreSharedKey != "" {
			bytes, err := base64.StdEncoding.DecodeString(option.PreSharedKey)
			if err != nil {
				return nil, E.Cause(err, "decode pre shared key")
			}
			option.PreSharedKey = hex.EncodeToString(bytes)
		}
	}
	outbound.option = option

	mtu := option.MTU
	if mtu == 0 {
		mtu = 1408
	}
	if len(outbound.localPrefixes) == 0 {
		return nil, E.New("missing local address")
	}
	outbound.tunDevice, err = wireguard.NewStackDevice(outbound.localPrefixes, uint32(mtu))
	if err != nil {
		return nil, E.Cause(err, "create WireGuard device")
	}
	logger := &device.Logger{
		Verbosef: func(format string, args ...interface{}) {
			log.SingLogger.Debug(fmt.Sprintf("[WG](%s) %s", option.Name, fmt.Sprintf(format, args...)))
		},
		Errorf: func(format string, args ...interface{}) {
			log.SingLogger.Error(fmt.Sprintf("[WG](%s) %s", option.Name, fmt.Sprintf(format, args...)))
		},
	}
	if option.AmneziaWGOption != nil {
		outbound.bind.SetParseReserved(false) // AmneziaWG don't need parse reserved
		outbound.device = amnezia.NewDevice(outbound.tunDevice, outbound.bind, logger, option.Workers)
	} else {
		outbound.device = device.NewDevice(outbound.tunDevice, outbound.bind, logger, option.Workers)
	}

	var has6 bool
	for _, address := range outbound.localPrefixes {
		if !address.Addr().Unmap().Is4() {
			has6 = true
			break
		}
	}

	if option.RemoteDnsResolve && len(option.Dns) > 0 {
		nss, err := dns.ParseNameServer(option.Dns)
		if err != nil {
			return nil, err
		}
		for i := range nss {
			nss[i].ProxyAdapter = outbound
		}
		outbound.resolver = dns.NewResolver(dns.Config{
			Main: nss,
			IPv6: has6,
		})
	}

	return outbound, nil
}

func (w *WireGuard) resolve(ctx context.Context, address M.Socksaddr) (netip.AddrPort, error) {
	if address.Addr.IsValid() {
		return address.AddrPort(), nil
	}
	udpAddr, err := resolveUDPAddr(ctx, "udp", address.String(), w.prefer)
	if err != nil {
		return netip.AddrPort{}, err
	}
	// net.ResolveUDPAddr maybe return 4in6 address, so unmap at here
	addrPort := udpAddr.AddrPort()
	return netip.AddrPortFrom(addrPort.Addr().Unmap(), addrPort.Port()), nil
}

func (w *WireGuard) init(ctx context.Context) error {
	err := w.init0(ctx)
	if err != nil {
		return err
	}
	w.updateServerAddr(ctx)
	return nil
}

func (w *WireGuard) init0(ctx context.Context) error {
	if w.initOk.Load() {
		return nil
	}
	w.initMutex.Lock()
	defer w.initMutex.Unlock()
	// double check like sync.Once
	if w.initOk.Load() {
		return nil
	}
	if w.initErr != nil {
		return w.initErr
	}

	w.bind.ResetReservedForEndpoint()
	w.serverAddrMap = make(map[M.Socksaddr]netip.AddrPort)
	ipcConf, err := w.genIpcConf(ctx, false)
	if err != nil {
		// !!! do not set initErr here !!!
		// let us can retry domain resolve in next time
		return err
	}

	if debug.Enabled {
		log.SingLogger.Trace(fmt.Sprintf("[WG](%s) created wireguard ipc conf: \n %s", w.option.Name, ipcConf))
	}
	err = w.device.IpcSet(ipcConf)
	if err != nil {
		w.initErr = E.Cause(err, "setup wireguard")
		return w.initErr
	}
	w.serverAddrTime.Store(time.Now())

	err = w.tunDevice.Start()
	if err != nil {
		w.initErr = err
		return w.initErr
	}

	w.initOk.Store(true)
	return nil
}

func (w *WireGuard) updateServerAddr(ctx context.Context) {
	if w.option.RefreshServerIPInterval != 0 && time.Since(w.serverAddrTime.Load()) > time.Second*time.Duration(w.option.RefreshServerIPInterval) {
		if w.serverAddrMutex.TryLock() {
			defer w.serverAddrMutex.Unlock()
			ipcConf, err := w.genIpcConf(ctx, true)
			if err != nil {
				log.Warnln("[WG](%s)UpdateServerAddr failed to generate wireguard ipc conf: %s", w.option.Name, err)
				return
			}
			err = w.device.IpcSet(ipcConf)
			if err != nil {
				log.Warnln("[WG](%s)UpdateServerAddr failed to update wireguard ipc conf: %s", w.option.Name, err)
				return
			}
			w.serverAddrTime.Store(time.Now())
		}
	}
}

func (w *WireGuard) genIpcConf(ctx context.Context, updateOnly bool) (string, error) {
	ipcConf := ""
	if !updateOnly {
		ipcConf += "private_key=" + w.option.PrivateKey + "\n"
		if w.option.AmneziaWGOption != nil {
			ipcConf += "jc=" + strconv.Itoa(w.option.AmneziaWGOption.JC) + "\n"
			ipcConf += "jmin=" + strconv.Itoa(w.option.AmneziaWGOption.JMin) + "\n"
			ipcConf += "jmax=" + strconv.Itoa(w.option.AmneziaWGOption.JMax) + "\n"
			ipcConf += "s1=" + strconv.Itoa(w.option.AmneziaWGOption.S1) + "\n"
			ipcConf += "s2=" + strconv.Itoa(w.option.AmneziaWGOption.S2) + "\n"
			ipcConf += "h1=" + strconv.FormatUint(uint64(w.option.AmneziaWGOption.H1), 10) + "\n"
			ipcConf += "h2=" + strconv.FormatUint(uint64(w.option.AmneziaWGOption.H2), 10) + "\n"
			ipcConf += "h3=" + strconv.FormatUint(uint64(w.option.AmneziaWGOption.H3), 10) + "\n"
			ipcConf += "h4=" + strconv.FormatUint(uint64(w.option.AmneziaWGOption.H4), 10) + "\n"
		}
	}
	if len(w.option.Peers) > 0 {
		for i, peer := range w.option.Peers {
			peerAddr := peer.Addr()
			destination, err := w.resolve(ctx, peerAddr)
			if err != nil {
				return "", E.Cause(err, "resolve endpoint domain for peer ", i)
			}
			if w.serverAddrMap[peerAddr] != destination {
				w.serverAddrMap[peerAddr] = destination
			} else if updateOnly {
				continue
			}

			if len(w.option.Peers) == 1 { // must call SetConnectAddr if isConnect == true
				w.bind.SetConnectAddr(destination)
			}
			ipcConf += "public_key=" + peer.PublicKey + "\n"
			if updateOnly {
				ipcConf += "update_only=true\n"
			}
			ipcConf += "endpoint=" + destination.String() + "\n"
			if len(peer.Reserved) > 0 {
				var reserved [3]uint8
				copy(reserved[:], w.option.Reserved)
				w.bind.SetReservedForEndpoint(destination, reserved)
			}
			if updateOnly {
				continue
			}
			if peer.PreSharedKey != "" {
				ipcConf += "preshared_key=" + peer.PreSharedKey + "\n"
			}
			for _, allowedIP := range peer.AllowedIPs {
				ipcConf += "allowed_ip=" + allowedIP + "\n"
			}
			if w.option.PersistentKeepalive != 0 {
				ipcConf += fmt.Sprintf("persistent_keepalive_interval=%d\n", w.option.PersistentKeepalive)
			}
		}
	} else {
		destination, err := w.resolve(ctx, w.connectAddr)
		if err != nil {
			return "", E.Cause(err, "resolve endpoint domain")
		}
		if w.serverAddrMap[w.connectAddr] != destination {
			w.serverAddrMap[w.connectAddr] = destination
		} else if updateOnly {
			return "", nil
		}
		w.bind.SetConnectAddr(destination) // must call SetConnectAddr if isConnect == true
		ipcConf += "public_key=" + w.option.PublicKey + "\n"
		if updateOnly {
			ipcConf += "update_only=true\n"
		}
		ipcConf += "endpoint=" + destination.String() + "\n"
		if updateOnly {
			return ipcConf, nil
		}
		if w.option.PreSharedKey != "" {
			ipcConf += "preshared_key=" + w.option.PreSharedKey + "\n"
		}
		var has4, has6 bool
		for _, address := range w.localPrefixes {
			if address.Addr().Is4() {
				has4 = true
			} else {
				has6 = true
			}
		}
		if has4 {
			ipcConf += "allowed_ip=0.0.0.0/0\n"
		}
		if has6 {
			ipcConf += "allowed_ip=::/0\n"
		}

		if w.option.PersistentKeepalive != 0 {
			ipcConf += fmt.Sprintf("persistent_keepalive_interval=%d\n", w.option.PersistentKeepalive)
		}
	}
	return ipcConf, nil
}

// Close implements C.ProxyAdapter
func (w *WireGuard) Close() error {
	if w.device != nil {
		w.device.Close()
	}
	return nil
}

func (w *WireGuard) DialContext(ctx context.Context, metadata *C.Metadata) (_ C.Conn, err error) {
	var conn net.Conn
	if err = w.init(ctx); err != nil {
		return nil, err
	}
	if !metadata.Resolved() || w.resolver != nil {
		r := resolver.DefaultResolver
		if w.resolver != nil {
			r = w.resolver
		}
		options := w.DialOptions()
		options = append(options, dialer.WithResolver(r))
		options = append(options, dialer.WithNetDialer(wgNetDialer{tunDevice: w.tunDevice}))
		conn, err = dialer.NewDialer(options...).DialContext(ctx, "tcp", metadata.RemoteAddress())
	} else {
		conn, err = w.tunDevice.DialContext(ctx, "tcp", M.SocksaddrFrom(metadata.DstIP, metadata.DstPort).Unwrap())
	}
	if err != nil {
		return nil, err
	}
	if conn == nil {
		return nil, E.New("conn is nil")
	}
	return NewConn(conn, w), nil
}

func (w *WireGuard) ListenPacketContext(ctx context.Context, metadata *C.Metadata) (_ C.PacketConn, err error) {
	var pc net.PacketConn
	if err = w.init(ctx); err != nil {
		return nil, err
	}
	if err = w.ResolveUDP(ctx, metadata); err != nil {
		return nil, err
	}
	pc, err = w.tunDevice.ListenPacket(ctx, M.SocksaddrFrom(metadata.DstIP, metadata.DstPort).Unwrap())
	if err != nil {
		return nil, err
	}
	if pc == nil {
		return nil, E.New("packetConn is nil")
	}
	return newPacketConn(pc, w), nil
}

func (w *WireGuard) ResolveUDP(ctx context.Context, metadata *C.Metadata) error {
	if (!metadata.Resolved() || w.resolver != nil) && metadata.Host != "" {
		r := resolver.DefaultResolver
		if w.resolver != nil {
			r = w.resolver
		}
		ip, err := resolver.ResolveIPWithResolver(ctx, metadata.Host, r)
		if err != nil {
			return fmt.Errorf("can't resolve ip: %w", err)
		}
		metadata.DstIP = ip
	}
	return nil
}

// IsL3Protocol implements C.ProxyAdapter
func (w *WireGuard) IsL3Protocol(metadata *C.Metadata) bool {
	return true
}
