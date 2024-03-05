package libcore

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/v2fly/v2ray-core/v5"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/protocol/udp"
	commonSerial "github.com/v2fly/v2ray-core/v5/common/serial"
	"github.com/v2fly/v2ray-core/v5/common/signal"
	"github.com/v2fly/v2ray-core/v5/features"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/extension"
	"github.com/v2fly/v2ray-core/v5/features/outbound"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/features/stats"
	"github.com/v2fly/v2ray-core/v5/infra/conf/serial"
	_ "github.com/v2fly/v2ray-core/v5/main/distro/minimal"
	"github.com/v2fly/v2ray-core/v5/proxy/vmess"
	vmessOutbound "github.com/v2fly/v2ray-core/v5/proxy/vmess/outbound"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/pipe"
)

func GetV2RayVersion() string {
	return core.Version()
}

type V2RayInstance struct {
	started         bool
	core            *core.Instance
	dispatcher      routing.Dispatcher
	router          routing.Router
	outboundManager outbound.Manager
	statsManager    stats.Manager
	observatory     features.TaggedFeatures
	dnsClient       dns.NewClient
}

func NewV2rayInstance() *V2RayInstance {
	return &V2RayInstance{}
}

func (instance *V2RayInstance) LoadConfig(content string) error {
	config, err := serial.LoadJSONConfig(strings.NewReader(content))
	if err != nil {
		if strings.HasSuffix(err.Error(), "geoip.dat: no such file or directory") {
			err = extractAssetName(geoipDat, true)
		} else if strings.HasSuffix(err.Error(), "not found in geoip.dat") {
			err = extractAssetName(geoipDat, false)
		} else if strings.HasSuffix(err.Error(), "geosite.dat: no such file or directory") {
			err = extractAssetName(geositeDat, true)
		} else if strings.HasSuffix(err.Error(), "not found in geosite.dat") {
			err = extractAssetName(geositeDat, false)
		}
		if err == nil {
			config, err = serial.LoadJSONConfig(strings.NewReader(content))
		}
	}
	if err != nil {
		return err
	}
	if config.Outbound != nil {
		for _, outbound := range config.Outbound {
			if outbound.ProxySettings == nil {
				continue
			}
			proxyConfig, err := commonSerial.GetInstanceOf(outbound.ProxySettings)
			if err != nil {
				continue
			}
			proxy, ok := proxyConfig.(*vmessOutbound.Config)
			if !ok {
				continue
			}
			var reset bool
			for _, endpoint := range proxy.Receiver {
				for _, user := range endpoint.User {
					if user.Account == nil {
						continue
					}
					accountConfig, err := commonSerial.GetInstanceOf(user.Account)
					if err != nil {
						continue
					}
					account, ok := accountConfig.(*vmess.Account)
					if !ok {
						continue
					}
					if account.AlterId > 0 {
						account.AlterId = 0
						user.Account = commonSerial.ToTypedMessage(account)
						reset = true
					}
				}
			}
			if reset {
				outbound.ProxySettings = commonSerial.ToTypedMessage(proxy)
			}
		}
	}

	c, err := core.New(config)
	if err != nil {
		return err
	}
	instance.core = c
	instance.statsManager = c.GetFeature(stats.ManagerType()).(stats.Manager)
	instance.router = c.GetFeature(routing.RouterType()).(routing.Router)
	instance.outboundManager = c.GetFeature(outbound.ManagerType()).(outbound.Manager)
	instance.dispatcher = c.GetFeature(routing.DispatcherType()).(routing.Dispatcher)
	instance.dnsClient = c.GetFeature(dns.ClientType()).(dns.NewClient)

	o := c.GetFeature(extension.ObservatoryType())
	if o != nil {
		instance.observatory = o.(features.TaggedFeatures)
	}
	return nil
}

func (instance *V2RayInstance) Start(errorHandler ErrorHandler) error {
	if instance.started {
		return errors.New("already started")
	}
	if instance.core == nil {
		return errors.New("not initialized")
	}
	instance.core.SetErrorHandler(func(err error) {
		errorHandler.HandleError(err.Error())
	})
	err := instance.core.Start()
	if err != nil {
		return err
	}
	instance.started = true
	return nil
}

func (instance *V2RayInstance) QueryStats(tag string, direct string) int64 {
	if instance.statsManager == nil {
		return 0
	}
	counter := instance.statsManager.GetCounter(fmt.Sprintf("outbound>>>%s>>>traffic>>>%s", tag, direct))
	if counter == nil {
		return 0
	}
	return counter.Set(0)
}

func (instance *V2RayInstance) Close() error {
	if instance.started {
		err := instance.core.Close()
		if err == nil {
			*instance = V2RayInstance{}
		}
		return err
	}
	return nil
}

func getLink(ctx context.Context) (*transport.Link, *transport.Link) {
	opt := pipe.OptionsFromContext(ctx)
	uplinkReader, uplinkWriter := pipe.New(opt...)
	downlinkReader, downlinkWriter := pipe.New(opt...)

	inboundLink := &transport.Link{
		Reader: downlinkReader,
		Writer: uplinkWriter,
	}

	outboundLink := &transport.Link{
		Reader: uplinkReader,
		Writer: downlinkWriter,
	}
	return inboundLink, outboundLink
}

func (instance *V2RayInstance) dialContext(ctx context.Context, destination net.Destination) (net.Conn, error) {
	if !instance.started {
		return nil, os.ErrInvalid
	}
	ctx = core.WithContext(ctx, instance.core)
	r, err := instance.dispatcher.Dispatch(ctx, destination)
	if err != nil {
		return nil, err
	}
	var readerOpt buf.ConnectionOption
	if destination.Network == net.Network_TCP {
		readerOpt = buf.ConnectionOutputMulti(r.Reader)
	} else {
		readerOpt = buf.ConnectionOutputMultiUDP(r.Reader)
	}
	return buf.NewConnection(buf.ConnectionInputMulti(r.Writer), readerOpt), nil
}

func (instance *V2RayInstance) dispatchContext(ctx context.Context, destination net.Destination, conn net.Conn) error {
	if !instance.started {
		return os.ErrInvalid
	}
	ctx = core.WithContext(ctx, instance.core)
	return instance.dispatcher.DispatchLink(ctx, destination, &transport.Link{
		Reader: buf.NewReader(conn),
		Writer: buf.NewWriter(conn),
	})
}

func (instance *V2RayInstance) dialUDP(ctx context.Context, destination net.Destination, timeout time.Duration) (packetConn, error) {
	if !instance.started {
		return nil, os.ErrInvalid
	}
	ctx, cancel := context.WithCancel(ctx)
	link, err := instance.dispatcher.Dispatch(ctx, destination)
	if err != nil {
		cancel()
		return nil, err
	}
	c := &dispatcherConn{
		dest:   destination,
		link:   link,
		ctx:    ctx,
		cancel: cancel,
		cache:  make(chan *udp.Packet, 16),
	}
	c.timer = signal.CancelAfterInactivity(ctx, func() {
		c.Close()
	}, timeout)
	go c.handleInput()
	return c, nil
}

func (instance *V2RayInstance) handleUDP(ctx context.Context, handler outbound.Handler, destination net.Destination, timeout time.Duration) packetConn {
	ctx, cancel := context.WithCancel(ctx)
	inboundLink, outboundLink := getLink(ctx)
	go handler.Dispatch(ctx, outboundLink)
	c := &dispatcherConn{
		dest:   destination,
		link:   inboundLink,
		ctx:    ctx,
		cancel: cancel,
		cache:  make(chan *udp.Packet, 16),
	}
	c.timer = signal.CancelAfterInactivity(ctx, func() {
		c.Close()
	}, timeout)
	go c.handleInput()
	return c
}

var _ packetConn = (*dispatcherConn)(nil)

type dispatcherConn struct {
	access sync.Mutex
	dest   net.Destination
	link   *transport.Link
	timer  *signal.ActivityTimer

	ctx    context.Context
	cancel context.CancelFunc
	closed bool
	cache  chan *udp.Packet
}

func (c *dispatcherConn) IsPipe() bool {
	return true
}

func (c *dispatcherConn) handleInput() {
	defer c.Close()
	for {
		select {
		case <-c.ctx.Done():
			return
		default:
		}

		mb, err := c.link.Reader.ReadMultiBuffer()
		if err != nil {
			buf.ReleaseMulti(mb)
			return
		}
		c.timer.Update()
		for _, buffer := range mb {
			if buffer.Len() <= 0 {
				continue
			}
			packet := udp.Packet{
				Payload: buffer,
			}
			if buffer.Endpoint == nil {
				packet.Source = c.dest
			} else {
				packet.Source = *buffer.Endpoint
			}
			if packet.Source.Address.Family().IsDomain() {
				packet.Source.Address = net.AnyIP
			}
			select {
			case c.cache <- &packet:
				continue
			case <-c.ctx.Done():
			default:
			}
			buffer.Release()
		}
	}
}

func (c *dispatcherConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	select {
	case <-c.ctx.Done():
		return 0, nil, io.EOF
	case packet := <-c.cache:
		n := copy(p, packet.Payload.Bytes())
		packet.Payload.Release()
		return n, &net.UDPAddr{
			IP:   packet.Source.Address.IP(),
			Port: int(packet.Source.Port),
		}, nil
	}
}

func (c *dispatcherConn) readFrom() (buffer *buf.Buffer, addr net.Addr, err error) {
	select {
	case <-c.ctx.Done():
		return nil, nil, io.EOF
	case packet, ok := <-c.cache:
		if !ok {
			return nil, nil, io.EOF
		}
		return packet.Payload, &net.UDPAddr{
			IP:   packet.Source.Address.IP(),
			Port: int(packet.Source.Port),
		}, nil
	}
}

func (c *dispatcherConn) WriteTo(p []byte, addr net.Addr) (n int, err error) {
	buffer := buf.New()
	buffer.Write(p)
	endpoint := net.DestinationFromAddr(addr)
	buffer.Endpoint = &endpoint
	err = c.link.Writer.WriteMultiBuffer(buf.MultiBuffer{buffer})
	if err != nil {
		buffer.Release()
		c.Close()
		return 0, err
	} else {
		c.timer.Update()
		n = len(p)
	}
	return
}

func (c *dispatcherConn) writeTo(buffer *buf.Buffer, addr net.Addr) (err error) {
	endpoint := net.DestinationFromAddr(addr)
	buffer.Endpoint = &endpoint
	err = c.link.Writer.WriteMultiBuffer(buf.MultiBuffer{buffer})
	if err != nil {
		buffer.Release()
		c.Close()
	} else {
		c.timer.Update()
	}
	return
}

func (c *dispatcherConn) RemoteAddr() net.Addr {
	return nil
}

func (c *dispatcherConn) LocalAddr() net.Addr {
	return &net.UDPAddr{
		IP:   []byte{0, 0, 0, 0},
		Port: 0,
	}
}

func (c *dispatcherConn) Close() error {
	if c.closed {
		return nil
	}
	c.closed = true

	c.cancel()
	_ = common.Interrupt(c.link.Reader)
	_ = common.Interrupt(c.link.Writer)
	close(c.cache)

	return nil
}

func (c *dispatcherConn) SetDeadline(t time.Time) error {
	return nil
}

func (c *dispatcherConn) SetReadDeadline(t time.Time) error {
	return nil
}

func (c *dispatcherConn) SetWriteDeadline(t time.Time) error {
	return nil
}
