package wireguard

import (
	"context"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"strings"
	"sync"

	"golang.zx2c4.com/wireguard/conn"
	"golang.zx2c4.com/wireguard/device"
	"gvisor.dev/gvisor/pkg/tcpip"
	"gvisor.dev/gvisor/pkg/tcpip/adapters/gonet"
	"gvisor.dev/gvisor/pkg/tcpip/header"

	core "github.com/v2fly/v2ray-core/v5"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/pingproto"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/common/signal"
	"github.com/v2fly/v2ray-core/v5/common/signal/done"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/policy"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/proxy"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

func init() {
	common.Must(common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		o := new(Client)
		err := core.RequireFeatures(ctx, func(dispatcher routing.Dispatcher, policyManager policy.Manager, dnsClient dns.Client) error {
			o.ctx = ctx
			o.dispatcher = dispatcher
			o.dnsClient = dnsClient
			o.init = done.New()
			return o.Init(config.(*Config), policyManager)
		})
		return o, err
	}))
}

var (
	_ proxy.Outbound          = (*Client)(nil)
	_ pingproto.ICMPInterface = (*Client)(nil)
	_ common.Closable         = (*Client)(nil)
)

type Client struct {
	sync.Mutex

	ctx           context.Context
	dispatcher    routing.Dispatcher
	sessionPolicy policy.Session
	dnsClient     dns.Client

	tun    *wireDevice
	dev    *device.Device
	dialer internet.Dialer

	init        *done.Instance
	destination net.Destination
	endpoint    *conn.StdNetEndpoint
	connection  *remoteConnection

	pingConn4   *pingConnWrapper
	pingConn6   *pingConnWrapper
	pingManager *pingproto.ClientManager
}

func (c *Client) Reset4() error {
	return nil
}

func (c *Client) Reset6() error {
	return nil
}

func (c *Client) Close() error {
	c.tun.Close()
	c.dev.Close()
	return nil
}

func (c *Client) Init(config *Config, policyManager policy.Manager) error {
	c.sessionPolicy = policyManager.ForLevel(config.UserLevel)
	c.destination = net.Destination{
		Network: config.Network,
		Address: config.Address.AsAddress(),
		Port:    net.Port(config.Port),
	}
	c.destination.Network = config.Network

	if c.destination.Network == net.Network_Unknown {
		c.destination.Network = net.Network_UDP
	}

	c.endpoint = &conn.StdNetEndpoint{
		Port: int(c.destination.Port),
	}

	if c.destination.Address.Family().IsDomain() {
		c.endpoint.IP = []byte{1, 0, 0, 1}
	} else {
		c.endpoint.IP = c.destination.Address.IP()
	}

	localAddress := make([]net.IP, len(config.LocalAddress))
	if len(localAddress) == 0 {
		return newError("empty local address")
	}
	for index, address := range config.LocalAddress {
		localAddress[index] = net.ParseIP(address)
	}

	var privateKey, peerPublicKey, preSharedKey string
	{
		decoder := base64.NewDecoder(base64.StdEncoding, strings.NewReader(config.PrivateKey))
		bytes, err := buf.ReadAllToBytes(decoder)
		if err != nil {
			return newError("failed to decode private key from base64: ", config.PrivateKey).Base(err)
		}
		privateKey = hex.EncodeToString(bytes)
	}
	{
		decoder := base64.NewDecoder(base64.StdEncoding, strings.NewReader(config.PeerPublicKey))
		bytes, err := buf.ReadAllToBytes(decoder)
		if err != nil {
			return newError("failed to decode peer public key from base64: ", config.PeerPublicKey).Base(err)
		}
		peerPublicKey = hex.EncodeToString(bytes)
	}
	if config.PreSharedKey != "" {
		decoder := base64.NewDecoder(base64.StdEncoding, strings.NewReader(config.PreSharedKey))
		bytes, err := buf.ReadAllToBytes(decoder)
		if err != nil {
			return newError("failed to decode pre share key from base64: ", config.PreSharedKey).Base(err)
		}
		preSharedKey = hex.EncodeToString(bytes)
	}
	ipcConf := "private_key=" + privateKey
	ipcConf += "\npublic_key=" + peerPublicKey
	ipcConf += "\nendpoint=" + c.endpoint.DstToString()

	if preSharedKey != "" {
		ipcConf += "\npreshared_key=" + preSharedKey
	}

	var has4, has6 bool

	for _, address := range localAddress {
		if address.To4() != nil {
			has4 = true
		} else {
			has6 = true
		}
	}

	if has4 {
		ipcConf += "\nallowed_ip=0.0.0.0/0"
	}

	if has6 {
		ipcConf += "\nallowed_ip=::/0"
	}

	mtu := int(config.Mtu)
	if mtu == 0 {
		mtu = 1450
	}

	c.pingManager = pingproto.NewClientManager(c)
	tun, err := newDevice(localAddress, mtu, c.pingManager)
	if err != nil {
		return newError("failed to create wireguard device").Base(err)
	}

	bind := &clientBind{c}
	dev := device.NewDevice(tun, bind, &device.Logger{
		Verbosef: func(format string, args ...interface{}) {
			newError(fmt.Sprintf(format, args...)).AtDebug().WriteToLog()
		},
		Errorf: func(format string, args ...interface{}) {
			newError(fmt.Sprintf(format, args...)).WriteToLog()
		},
	})

	newError("created wireguard ipc conf: ", ipcConf).AtDebug().WriteToLog()

	err = dev.IpcSet(ipcConf)
	if err != nil {
		return newError("failed to set wireguard ipc conf").Base(err)
	}

	c.tun = tun
	c.dev = dev

	c.pingConn4 = &pingConnWrapper{c.tun.writePing4}
	c.pingConn6 = &pingConnWrapper{c.tun.writePing6}

	return nil
}

func (c *Client) Process(ctx context.Context, link *transport.Link, dialer internet.Dialer) error {
	if c.dialer == nil {
		c.dialer = dialer
	}
	c.init.Close()

	outbound := session.OutboundFromContext(ctx)
	if outbound == nil || !outbound.Target.IsValid() {
		return newError("target not specified")
	}
	destination := outbound.Target

	if destination.Address.Family().IsDomain() {
		ips, err := c.dnsClient.LookupIP(destination.Address.Domain())
		if err != nil {
			return newError("failed to lookup ip addresses for domain ", destination.Address.Domain()).Base(err)
		}
		destination.Address = net.IPAddress(ips[0])
	}

	var conn internet.Connection

	if destination.Network == net.Network_UDP && destination.Port == 7 {
		conn = c.pingManager.GetClient(destination).CreateConnection()
	} else {
		bind := tcpip.FullAddress{
			NIC: defaultNIC,
		}
		address := tcpip.FullAddress{
			NIC:  defaultNIC,
			Addr: tcpip.Address(destination.Address.IP()),
			Port: uint16(destination.Port),
		}

		var network tcpip.NetworkProtocolNumber
		if destination.Address.Family().IsIPv4() {
			network = header.IPv4ProtocolNumber
			bind.Addr = c.tun.addr4
		} else {
			network = header.IPv6ProtocolNumber
			bind.Addr = c.tun.addr6
		}

		var err error
		switch destination.Network {
		case net.Network_TCP:
			conn, err = gonet.DialTCPWithBind(ctx, c.tun.stack, bind, address, network)
		case net.Network_UDP:
			var wireConn *gonet.UDPConn
			wireConn, err = gonet.DialUDP(c.tun.stack, &bind, &address, network)
			if err == nil {
				conn = &udpConn{wireConn}
			}
		}
		if err != nil {
			return newError("failed to dial to virtual device").Base(err)
		}
	}

	defer conn.Close()

	ctx, cancel := context.WithCancel(ctx)
	timer := signal.CancelAfterInactivity(ctx, cancel, c.sessionPolicy.Timeouts.ConnectionIdle)
	ctx = policy.ContextWithBufferPolicy(ctx, c.sessionPolicy.Buffer)

	uplink := func() error {
		defer timer.SetTimeout(c.sessionPolicy.Timeouts.UplinkOnly)
		return buf.Copy(link.Reader, buf.NewWriter(conn), buf.UpdateActivity(timer))
	}

	downlink := func() error {
		defer timer.SetTimeout(c.sessionPolicy.Timeouts.DownlinkOnly)
		return buf.Copy(buf.NewReader(conn), link.Writer, buf.UpdateActivity(timer))
	}

	if err := task.Run(ctx, uplink, downlink); err != nil {
		common.Interrupt(link.Reader)
		common.Interrupt(link.Writer)
		return newError("connection ends").Base(err)
	}

	return nil
}

func (c *Client) IPv4Connection() net.PacketConn {
	return c.pingConn4
}

func (c *Client) IPv6Connection() net.PacketConn {
	return c.pingConn6
}

func (c *Client) NeedChecksum() bool {
	return false
}

type remoteConnection struct {
	internet.Connection
	done *done.Instance
}

func (r remoteConnection) Close() error {
	if !r.done.Done() {
		r.done.Close()
	}
	return r.Connection.Close()
}

func (c *Client) connect() (*remoteConnection, error) {
	if c.dialer == nil {
		<-c.init.Wait()
	}

	if c := c.connection; c != nil && !c.done.Done() {
		return c, nil
	}

	c.Lock()
	defer c.Unlock()

	if c := c.connection; c != nil && !c.done.Done() {
		return c, nil
	}

	conn, err := c.dialer.Dial(core.ToBackgroundDetachedContext(c.ctx), c.destination)
	if err == nil {
		c.connection = &remoteConnection{
			conn,
			done.New(),
		}
	}

	return c.connection, err
}

var _ conn.Bind = (*clientBind)(nil)

type clientBind struct {
	*Client
}

func (o *clientBind) Open(uint16) (fns []conn.ReceiveFunc, actualPort uint16, err error) {
	return []conn.ReceiveFunc{o.Receive}, 0, nil
}

func (o *clientBind) Receive(b []byte) (n int, ep conn.Endpoint, err error) {
	var c *remoteConnection
	c, err = o.connect()
	if err != nil {
		return
	}
	n, err = c.Read(b)
	if err != nil {
		common.Close(c)
	} else {
		ep = o.endpoint
	}
	return
}

func (o *clientBind) Close() error {
	o.Lock()
	defer o.Unlock()

	c := o.connection
	if c != nil {
		common.Close(c)
	}

	return nil
}

func (o *clientBind) SetMark(uint32) error {
	return nil
}

func (o *clientBind) Send(b []byte, _ conn.Endpoint) (err error) {
	var c *remoteConnection
	c, err = o.connect()
	if err != nil {
		return
	}
	_, err = c.Write(b)
	if err != nil {
		common.Close(c)
	}
	return err
}

func (o *clientBind) ParseEndpoint(string) (conn.Endpoint, error) {
	return o.endpoint, nil
}

type udpConn struct {
	*gonet.UDPConn
}

func (c *udpConn) ReadMultiBuffer() (buf.MultiBuffer, error) {
	buffer := buf.New()
	n, addr, err := c.ReadFrom(buffer.Extend(buf.Size))
	if err != nil {
		return nil, err
	}
	if addr != nil {
		endpoint := net.DestinationFromAddr(addr)
		buffer.Endpoint = &endpoint
	}
	buffer.Resize(0, int32(n))
	return buf.MultiBuffer{buffer}, nil
}

func (c *udpConn) WriteMultiBuffer(mb buf.MultiBuffer) error {
	defer buf.ReleaseMulti(mb)
	for _, buffer := range mb {
		var addr net.Addr
		if buffer.Endpoint != nil {
			addr = buffer.Endpoint.UDPAddr()
		}
		if _, err := c.WriteTo(buffer.Bytes(), addr); err != nil {
			return err
		}
	}
	return nil
}
