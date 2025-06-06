package sing

import (
	"context"
	"errors"
	"net"
	"net/netip"
	"sync"
	"time"

	"github.com/metacubex/mihomo/adapter/inbound"
	"github.com/metacubex/mihomo/adapter/outbound"
	N "github.com/metacubex/mihomo/common/net"
	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/log"

	mux "github.com/metacubex/sing-mux"
	vmess "github.com/metacubex/sing-vmess"
	"github.com/metacubex/sing/common"
	"github.com/metacubex/sing/common/buf"
	"github.com/metacubex/sing/common/bufio"
	"github.com/metacubex/sing/common/bufio/deadline"
	E "github.com/metacubex/sing/common/exceptions"
	M "github.com/metacubex/sing/common/metadata"
	"github.com/metacubex/sing/common/network"
	"github.com/metacubex/sing/common/uot"
)

const UDPTimeout = 5 * time.Minute

type ListenerConfig struct {
	Tunnel     C.Tunnel
	Type       C.Type
	Additions  []inbound.Addition
	UDPTimeout time.Duration
	MuxOption  MuxOption
}

type MuxOption struct {
	Padding bool          `yaml:"padding" json:"padding,omitempty"`
	Brutal  BrutalOptions `yaml:"brutal" json:"brutal,omitempty"`
}

type BrutalOptions struct {
	Enabled bool   `yaml:"enabled" json:"enabled"`
	Up      string `yaml:"up" json:"up,omitempty"`
	Down    string `yaml:"down" json:"down,omitempty"`
}

type ListenerHandler struct {
	ListenerConfig
	muxService *mux.Service
}

func UpstreamMetadata(metadata M.Metadata) M.Metadata {
	return M.Metadata{
		Source:      metadata.Source,
		Destination: metadata.Destination,
	}
}

func ConvertMetadata(metadata *C.Metadata) M.Metadata {
	return M.Metadata{
		Protocol:    metadata.Type.String(),
		Source:      M.SocksaddrFrom(metadata.SrcIP, metadata.SrcPort),
		Destination: M.ParseSocksaddrHostPort(metadata.String(), metadata.DstPort),
	}
}

func NewListenerHandler(lc ListenerConfig) (h *ListenerHandler, err error) {
	h = &ListenerHandler{ListenerConfig: lc}
	h.muxService, err = mux.NewService(mux.ServiceOptions{
		NewStreamContext: func(ctx context.Context, conn net.Conn) context.Context {
			return ctx
		},
		Logger:  log.SingInfoToDebugLogger, // convert sing-mux info log to debug
		Handler: h,
		Padding: lc.MuxOption.Padding,
		Brutal: mux.BrutalOptions{
			Enabled:    lc.MuxOption.Brutal.Enabled,
			SendBPS:    outbound.StringToBps(lc.MuxOption.Brutal.Up),
			ReceiveBPS: outbound.StringToBps(lc.MuxOption.Brutal.Down),
		},
	})
	return
}

func (h *ListenerHandler) IsSpecialFqdn(fqdn string) bool {
	switch fqdn {
	case mux.Destination.Fqdn,
		vmess.MuxDestination.Fqdn,
		uot.MagicAddress,
		uot.LegacyMagicAddress:
		return true
	default:
		return false
	}
}

func (h *ListenerHandler) ParseSpecialFqdn(ctx context.Context, conn net.Conn, metadata M.Metadata) error {
	switch metadata.Destination.Fqdn {
	case mux.Destination.Fqdn:
		return h.muxService.NewConnection(ctx, conn, UpstreamMetadata(metadata))
	case vmess.MuxDestination.Fqdn:
		return vmess.HandleMuxConnection(ctx, conn, h)
	case uot.MagicAddress:
		request, err := uot.ReadRequest(conn)
		if err != nil {
			return E.Cause(err, "read UoT request")
		}
		metadata.Destination = request.Destination
		return h.NewPacketConnection(ctx, uot.NewConn(conn, *request), metadata)
	case uot.LegacyMagicAddress:
		metadata.Destination = M.Socksaddr{Addr: netip.IPv4Unspecified()}
		return h.NewPacketConnection(ctx, uot.NewConn(conn, uot.Request{}), metadata)
	}
	return errors.New("not special fqdn")
}

func (h *ListenerHandler) NewConnection(ctx context.Context, conn net.Conn, metadata M.Metadata) error {
	if h.IsSpecialFqdn(metadata.Destination.Fqdn) {
		return h.ParseSpecialFqdn(ctx, conn, metadata)
	}

	if deadline.NeedAdditionalReadDeadline(conn) {
		conn = N.NewDeadlineConn(conn) // conn from sing should check NeedAdditionalReadDeadline
	}

	cMetadata := &C.Metadata{
		NetWork: C.TCP,
		Type:    h.Type,
	}
	if metadata.Source.IsIP() && metadata.Source.Fqdn == "" {
		cMetadata.RawSrcAddr = metadata.Source.Unwrap().TCPAddr()
	}
	if metadata.Destination.IsIP() && metadata.Destination.Fqdn == "" {
		cMetadata.RawDstAddr = metadata.Destination.Unwrap().TCPAddr()
	}
	inbound.ApplyAdditions(cMetadata, inbound.WithDstAddr(metadata.Destination), inbound.WithSrcAddr(metadata.Source), inbound.WithInAddr(conn.LocalAddr()))
	inbound.ApplyAdditions(cMetadata, h.Additions...)
	inbound.ApplyAdditions(cMetadata, getAdditions(ctx)...)

	h.Tunnel.HandleTCPConn(conn, cMetadata) // this goroutine must exit after conn unused
	return nil
}

func (h *ListenerHandler) NewPacketConnection(ctx context.Context, conn network.PacketConn, metadata M.Metadata) error {
	defer func() { _ = conn.Close() }()
	mutex := sync.Mutex{}
	writer := bufio.NewNetPacketWriter(conn) // a new interface to set nil in defer
	defer func() {
		mutex.Lock() // this goroutine must exit after all conn.WritePacket() is not running
		defer mutex.Unlock()
		writer = nil
	}()
	rwOptions := network.ReadWaitOptions{}
	readWaiter, isReadWaiter := bufio.CreatePacketReadWaiter(conn)
	if isReadWaiter {
		readWaiter.InitializeReadWaiter(rwOptions)
	}
	for {
		var (
			buff *buf.Buffer
			dest M.Socksaddr
			err  error
		)
		if isReadWaiter {
			buff, dest, err = readWaiter.WaitReadPacket()
		} else {
			buff = rwOptions.NewPacketBuffer()
			dest, err = conn.ReadPacket(buff)
			if buff != nil {
				rwOptions.PostReturn(buff)
			}
		}
		if err != nil {
			buff.Release()
			if ShouldIgnorePacketError(err) {
				break
			}
			return err
		}
		cPacket := &packet{
			writer: &writer,
			mutex:  &mutex,
			rAddr:  metadata.Source.UDPAddr(),
			lAddr:  conn.LocalAddr(),
			buff:   buff,
		}
		if lAddr := getInAddr(ctx); lAddr != nil {
			cPacket.lAddr = lAddr
		}
		h.handlePacket(ctx, cPacket, metadata.Source, dest)
	}
	return nil
}

type localAddr interface {
	LocalAddr() net.Addr
}

func (h *ListenerHandler) NewPacket(ctx context.Context, key netip.AddrPort, buffer *buf.Buffer, metadata M.Metadata, init func(natConn network.PacketConn) network.PacketWriter) {
	writer := bufio.NewNetPacketWriter(init(nil))
	mutex := sync.Mutex{}
	cPacket := &packet{
		writer: &writer,
		mutex:  &mutex,
		rAddr:  metadata.Source.UDPAddr(),
		buff:   buffer,
	}
	if conn, ok := common.Cast[localAddr](writer); ok {
		cPacket.rAddr = conn.LocalAddr()
	} else {
		cPacket.rAddr = metadata.Source.UDPAddr() // tun does not have real inAddr
	}
	h.handlePacket(ctx, cPacket, metadata.Source, metadata.Destination)
}

func (h *ListenerHandler) handlePacket(ctx context.Context, cPacket *packet, source M.Socksaddr, destination M.Socksaddr) {
	cMetadata := &C.Metadata{
		NetWork: C.UDP,
		Type:    h.Type,
	}
	if source.IsIP() && source.Fqdn == "" {
		cMetadata.RawSrcAddr = source.Unwrap().UDPAddr()
	}
	if destination.IsIP() && destination.Fqdn == "" {
		cMetadata.RawDstAddr = destination.Unwrap().UDPAddr()
	}
	inbound.ApplyAdditions(cMetadata, inbound.WithDstAddr(destination), inbound.WithSrcAddr(source), inbound.WithInAddr(cPacket.InAddr()))
	inbound.ApplyAdditions(cMetadata, h.Additions...)
	inbound.ApplyAdditions(cMetadata, getAdditions(ctx)...)

	h.Tunnel.HandleUDPPacket(cPacket, cMetadata)
}

func (h *ListenerHandler) NewError(ctx context.Context, err error) {
	log.Warnln("%s listener get error: %+v", h.Type.String(), err)
}

func (h *ListenerHandler) TypeMutation(typ C.Type) *ListenerHandler {
	handler := *h
	handler.Type = typ
	return &handler
}

func ShouldIgnorePacketError(err error) bool {
	// ignore simple error
	if E.IsTimeout(err) || E.IsClosed(err) || E.IsCanceled(err) {
		return true
	}
	return false
}

type packet struct {
	writer *network.NetPacketWriter
	mutex  *sync.Mutex
	rAddr  net.Addr
	lAddr  net.Addr
	buff   *buf.Buffer
}

func (c *packet) Data() []byte {
	return c.buff.Bytes()
}

// WriteBack wirtes UDP packet with source(ip, port) = `addr`
func (c *packet) WriteBack(b []byte, addr net.Addr) (n int, err error) {
	if addr == nil {
		err = errors.New("address is invalid")
		return
	}

	c.mutex.Lock()
	defer c.mutex.Unlock()
	conn := *c.writer
	if conn == nil {
		err = errors.New("writeBack to closed connection")
		return
	}

	return conn.WriteTo(b, addr)
}

// LocalAddr returns the source IP/Port of UDP Packet
func (c *packet) LocalAddr() net.Addr {
	return c.rAddr
}

func (c *packet) Drop() {
	c.buff.Release()
}

func (c *packet) InAddr() net.Addr {
	return c.lAddr
}
