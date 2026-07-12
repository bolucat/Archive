package outbound

import (
	"context"
	"net"
	"net/netip"
	"strconv"
	"time"

	"github.com/metacubex/mihomo/component/ca"
	"github.com/metacubex/mihomo/component/dialer"
	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/ntp"
	"github.com/metacubex/mihomo/transport/shadowquic"
	"github.com/metacubex/mihomo/transport/tuic"

	"github.com/metacubex/jls-quic-go"
	"github.com/metacubex/jls-tls"
)

type ShadowQuic struct {
	*Base
	option *ShadowQuicOption
	client *shadowquic.Client

	quicConfig *quic.Config
	tlsConfig  *tls.Config
}

type ShadowQuicOption struct {
	BasicOption
	Name                 string   `proxy:"name"`
	Server               string   `proxy:"server"`
	Port                 int      `proxy:"port"`
	Username             string   `proxy:"username,omitempty"`
	Password             string   `proxy:"password,omitempty"`
	SNI                  string   `proxy:"sni,omitempty"`
	ALPN                 []string `proxy:"alpn,omitempty"`
	QUICVersions         []string `proxy:"quic-versions,omitempty"`
	UDPOverStream        bool     `proxy:"udp-over-stream,omitempty"`
	ZeroRTT              bool     `proxy:"zero-rtt,omitempty"`
	KeepAliveInterval    int      `proxy:"keep-alive-interval,omitempty"`
	CongestionController string   `proxy:"congestion-controller,omitempty"`
	CWND                 int      `proxy:"cwnd,omitempty"`
	BBRProfile           string   `proxy:"bbr-profile,omitempty"`
	ReceiveWindowConn    int      `proxy:"recv-window-conn,omitempty"`
	ReceiveWindow        int      `proxy:"recv-window,omitempty"`
	DisableMTUDiscovery  bool     `proxy:"disable-mtu-discovery,omitempty"`
	MaxDatagramFrameSize int      `proxy:"max-datagram-frame-size,omitempty"`
	MaxOpenStreams       int      `proxy:"max-open-streams,omitempty"`
}

func (s *ShadowQuic) DialContext(ctx context.Context, metadata *C.Metadata) (C.Conn, error) {
	conn, err := s.client.DialContext(ctx, metadata)
	if err != nil {
		return nil, err
	}
	return NewConn(conn, s), nil
}

func (s *ShadowQuic) ListenPacketContext(ctx context.Context, metadata *C.Metadata) (C.PacketConn, error) {
	if err := s.ResolveUDP(ctx, metadata); err != nil {
		return nil, err
	}
	pc, err := s.client.ListenPacket(ctx, metadata)
	if err != nil {
		return nil, err
	}
	return NewPacketConn(pc, s), nil
}

func (s *ShadowQuic) Close() error {
	if s.client != nil {
		return s.client.Close()
	}
	return nil
}

func (s *ShadowQuic) ProxyInfo() C.ProxyInfo {
	info := s.Base.ProxyInfo()
	info.DialerProxy = s.option.DialerProxy
	return info
}

func NewShadowQuic(option ShadowQuicOption) (*ShadowQuic, error) {
	addr := net.JoinHostPort(option.Server, strconv.Itoa(option.Port))
	serverName := option.SNI
	if serverName == "" {
		serverName = option.Server
	}

	tlsConfig := &tls.Config{
		ServerName: serverName,
		MinVersion: tls.VersionTLS13,
		Time:       ntp.Now,
		RootCAs:    ca.GetCertPool(),
	}
	if option.ALPN != nil {
		tlsConfig.NextProtos = option.ALPN
	} else {
		tlsConfig.NextProtos = []string{"h3"}
	}
	tlsConfig.JLSConfig = &tls.JLSConfig{
		Enable: true,
		User: tls.JLSUser{
			Username: option.Username,
			Password: option.Password,
		},
	}
	if option.ZeroRTT {
		// DialEarly can only send 0-RTT data after TLS has cached a session
		// ticket from an earlier connection to this server.
		tlsConfig.ClientSessionCache = tls.NewLRUClientSessionCache(1)
	}

	if option.MaxDatagramFrameSize == 0 {
		option.MaxDatagramFrameSize = 1400
	}
	if option.MaxOpenStreams == 0 {
		option.MaxOpenStreams = 1024
	}
	if option.CWND == 0 {
		option.CWND = 32
	}

	quicVersions := shadowquic.DefaultQUICVersions()
	if len(option.QUICVersions) > 0 {
		parsedVersions, _, err := shadowquic.ParseQUICVersionProfile(option.QUICVersions)
		if err != nil {
			return nil, err
		}
		quicVersions = parsedVersions
	}

	quicConfig := &quic.Config{
		Versions:                       quicVersions,
		InitialStreamReceiveWindow:     uint64(option.ReceiveWindowConn),
		MaxStreamReceiveWindow:         uint64(option.ReceiveWindowConn),
		InitialConnectionReceiveWindow: uint64(option.ReceiveWindow),
		MaxConnectionReceiveWindow:     uint64(option.ReceiveWindow),
		MaxIncomingStreams:             int64(option.MaxOpenStreams),
		MaxIncomingUniStreams:          int64(option.MaxOpenStreams),
		DisablePathMTUDiscovery:        option.DisableMTUDiscovery,
		MaxDatagramFrameSize:           int64(option.MaxDatagramFrameSize),
		EnableDatagrams:                true,
	}
	if option.KeepAliveInterval > 0 {
		quicConfig.KeepAlivePeriod = time.Duration(option.KeepAliveInterval) * time.Millisecond
	}
	if option.ReceiveWindowConn == 0 {
		quicConfig.InitialStreamReceiveWindow = tuic.DefaultStreamReceiveWindow / 10
		quicConfig.MaxStreamReceiveWindow = tuic.DefaultStreamReceiveWindow
	}
	if option.ReceiveWindow == 0 {
		quicConfig.InitialConnectionReceiveWindow = tuic.DefaultConnectionReceiveWindow / 10
		quicConfig.MaxConnectionReceiveWindow = tuic.DefaultConnectionReceiveWindow
	}

	outbound := &ShadowQuic{
		Base: NewBase(BaseOption{
			Name:         option.Name,
			Addr:         addr,
			Type:         C.ShadowQuic,
			ProviderName: option.ProviderName,
			UDP:          true,
			Interface:    option.Interface,
			RoutingMark:  option.RoutingMark,
			Prefer:       option.IPVersion,
		}),
		option:     &option,
		quicConfig: quicConfig,
		tlsConfig:  tlsConfig,
	}
	outbound.dialer = option.NewDialer(outbound.DialOptions())
	outbound.client = shadowquic.NewClient(&shadowquic.ClientOption{
		UDPOverStream: option.UDPOverStream,
		Dial: func(ctx context.Context) (*quic.Conn, error) {
			_, quicConn, err := dialShadowQuic(ctx, outbound.addr, outbound.DialOptions(), outbound.dialer, outbound.tlsConfig, outbound.quicConfig, option.ZeroRTT)
			if err != nil {
				return nil, err
			}
			shadowquic.SetCongestionController(quicConn, option.CongestionController, option.CWND, option.BBRProfile)
			return quicConn, nil
		},
	})

	return outbound, nil
}

type shadowQuicPacketDialer interface {
	ListenPacket(ctx context.Context, network, address string, rAddrPort netip.AddrPort) (net.PacketConn, error)
}

func dialShadowQuic(ctx context.Context, address string, opts []dialer.Option, pDialer shadowQuicPacketDialer, tlsConf *tls.Config, conf *quic.Config, early bool) (net.PacketConn, *quic.Conn, error) {
	monitorAuthEarly := early
	if _, err := netip.ParseAddrPort(address); err != nil {
		// A hostname can resolve to multiple candidates. Authenticate each one
		// before the dialer selects it, so a failed candidate can fall back.
		monitorAuthEarly = false
	}
	d := dialer.NewDialer(
		dialer.WithOptions(opts...),
		dialer.WithNetDialer(dialer.NetDialerFunc(func(ctx context.Context, network, address string) (net.Conn, error) {
			addrPort, err := netip.ParseAddrPort(address)
			if err != nil {
				return nil, err
			}
			udpAddr := net.UDPAddrFromAddrPort(addrPort)
			packetConn, err := pDialer.ListenPacket(ctx, "udp", "", udpAddr.AddrPort())
			if err != nil {
				return nil, err
			}
			transport := quic.Transport{Conn: packetConn}
			transport.SetCreatedConn(true)
			transport.SetSingleUse(true)

			var quicConn *quic.Conn
			if early {
				quicConn, err = transport.DialEarly(ctx, udpAddr, tlsConf, conf)
			} else {
				quicConn, err = transport.Dial(ctx, udpAddr, tlsConf, conf)
			}
			if err != nil {
				_ = packetConn.Close()
				return nil, err
			}
			if err := monitorShadowQuicJLSAuth(quicConn, packetConn, tlsConf, monitorAuthEarly); err != nil {
				return nil, err
			}
			return shadowQuicNetConn{Conn: quicConn, pc: packetConn}, nil
		})),
	)
	c, err := d.DialContext(ctx, "udp", address)
	if err != nil {
		return nil, nil, err
	}
	nc := c.(shadowQuicNetConn)
	return nc.pc, nc.Conn, nil
}

func monitorShadowQuicJLSAuth(quicConn *quic.Conn, packetConn net.PacketConn, tlsConf *tls.Config, early bool) error {
	if tlsConf == nil || tlsConf.JLSConfig == nil || !tlsConf.JLSConfig.Enable {
		return nil
	}
	closeConn := func() {
		_ = quicConn.CloseWithError(0, "")
		_ = packetConn.Close()
	}
	checkAuth := func() bool {
		return quicConn.ConnectionState().TLS.JLS.Authenticated
	}
	if !early {
		if checkAuth() {
			return nil
		}
		closeConn()
		return tls.ErrJLSAuthFailed
	}
	go func() {
		select {
		case <-quicConn.HandshakeComplete():
			if !checkAuth() {
				closeConn()
			}
		case <-quicConn.Context().Done():
		}
	}()
	return nil
}

type shadowQuicNetConn struct {
	*quic.Conn
	pc net.PacketConn
}

func (q shadowQuicNetConn) Close() error {
	err := q.Conn.CloseWithError(0, "")
	_ = q.pc.Close()
	return err
}

func (q shadowQuicNetConn) Read([]byte) (int, error) {
	panic("should not call Read on shadowQuicNetConn")
}

func (q shadowQuicNetConn) Write([]byte) (int, error) {
	panic("should not call Write on shadowQuicNetConn")
}

func (q shadowQuicNetConn) SetDeadline(time.Time) error {
	panic("should not call SetDeadline on shadowQuicNetConn")
}

func (q shadowQuicNetConn) SetReadDeadline(time.Time) error {
	panic("should not call SetReadDeadline on shadowQuicNetConn")
}

func (q shadowQuicNetConn) SetWriteDeadline(time.Time) error {
	panic("should not call SetWriteDeadline on shadowQuicNetConn")
}

var _ net.Conn = shadowQuicNetConn{}
