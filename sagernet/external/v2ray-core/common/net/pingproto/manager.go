package pingproto

import (
	"context"
	"io"
	"os"
	"sync"
	"time"

	"gvisor.dev/gvisor/pkg/tcpip/header"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/errors"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/signal"
	"github.com/v2fly/v2ray-core/v5/features/ping"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

func init() {
	common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		manager := &PingManager{}
		return manager, manager.Init(config.(*Config))
	})
	common.RegisterConfig((*SimplifiedConfig)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		simplifiedConfig := config.(*SimplifiedConfig)
		c := Config{
			Gateway4:    simplifiedConfig.Gateway4,
			Gateway6:    simplifiedConfig.Gateway6,
			DisableIPv6: simplifiedConfig.DisableIPv6,
		}
		switch simplifiedConfig.Protocol {
		case "unprivileged":
			c.Protocol = Protocol_Unprivileged
		case "default", "":
			c.Protocol = Protocol_Default
		default:
			return nil, newError("unknown icmp listen protocol ", c.Protocol)
		}
		manager := &PingManager{}
		return manager, manager.Init(&c)
	})
}

var _ ping.Manager = (*PingManager)(nil)

type PingManager struct {
	access       sync.Mutex
	disableIPv6  bool
	unprivileged bool
	network4     string
	network6     string
	listen4      string
	listen6      string
	protocol4    string
	protocol6    string
	icmp4Conn    net.PacketConn
	icmp6Conn    net.PacketConn
	clientTable  sync.Map
	lockTable    sync.Map
	id           uint16
}

func (m *PingManager) Init(config *Config) error {
	m.disableIPv6 = config.DisableIPv6
	switch config.Protocol {
	case Protocol_Unprivileged:
		m.network4 = "udp4"
		m.network6 = "udp6"
		m.unprivileged = true
	case Protocol_Default:
		m.network4 = "ip4:icmp"
		m.network6 = "ip6:ipv6-icmp"
	}
	m.listen4, m.listen6 = config.Gateway4, config.Gateway6
	if m.listen4 == "" {
		m.listen4 = "0.0.0.0"
	}
	if m.listen6 == "" {
		m.listen6 = "::"
	}

	return nil
}

func (m *PingManager) Type() interface{} {
	return ping.ManagerType()
}

func (m *PingManager) Start() error {
	return nil
}

func (m *PingManager) Close() error {
	return errors.Combine(
		common.Close(m.icmp4Conn),
		common.Close(m.icmp6Conn),
	)
}

func (m *PingManager) Dial(destination net.Destination) (internet.Connection, error) {
	if destination.Address.Family().IsIPv4() {
		if m.icmp4Conn == nil {
			m.access.Lock()
			if m.icmp4Conn == nil {
				conn, err := ListenPacket(m.network4, m.listen4)
				if err != nil {
					m.access.Unlock()
					return nil, newError("failed to listen icmp on ", m.listen4).Base(err)
				}
				m.icmp4Conn = conn
				go m.loop4()
				newError("icmpv4 connection created").AtDebug().WriteToLog()
			}
			m.access.Unlock()
		}
	} else if m.disableIPv6 {
		return nil, newError("ipv6 ping disabled")
	} else if m.icmp6Conn == nil {
		m.access.Lock()
		if m.icmp6Conn == nil {
			conn, err := ListenPacket(m.network6, m.listen6)
			if err != nil {
				m.access.Unlock()
				return nil, newError("failed to listen icmp6 on ", m.listen6).Base(err)
			}
			m.icmp6Conn = conn
			go m.loop6()
			newError("icmpv6 connection created").AtDebug().WriteToLog()
		}
		m.access.Unlock()
	}
	return m.getClient(destination).createConnection(), nil
}

func (m *PingManager) getClient(destination net.Destination) *pingClient {
	key := destination.Address.String()
	if clientI, loaded := m.clientTable.Load(key); loaded {
		return clientI.(*pingClient)
	}
	var cond *sync.Cond
	iCond, loaded := m.lockTable.LoadOrStore(key, sync.NewCond(&sync.Mutex{}))
	cond = iCond.(*sync.Cond)
	if loaded {
		cond.L.Lock()
		cond.Wait()
		defer cond.L.Unlock()

		if clientI, loaded := m.clientTable.Load(key); loaded {
			return clientI.(*pingClient)
		} else {
			panic("unable to load connection from ping manager")
		}
	}
	ctx, cancel := context.WithCancel(context.Background())
	m.id++
	client := &pingClient{
		PingManager: m,
		ctx:         ctx,
		id:          m.id,
		timer: signal.CancelAfterInactivity(ctx, func() {
			cancel()
			m.clientTable.Delete(key)
		}, 30*time.Second),
		dest: destination,
	}
	m.clientTable.Store(key, client)
	m.lockTable.Delete(key)
	cond.Broadcast()
	return client
}

func (m *PingManager) loop4() {
	buffer := buf.StackNew()
	defer buffer.Release()
	for {
		buffer.Clear()
		_, err := buffer.ReadFromPacketConn(m.icmp4Conn)
		if err != nil {
			if err != os.ErrClosed {
				newError("icmp4 receive failed").Base(err)
				m.Close()
			}
			break
		}
		if buffer.Endpoint == nil {
			newError("nil icmp4 endpoint").WriteToLog()
			continue
		}
		client := m.getClient(*buffer.Endpoint)
		hdr := header.ICMPv4(buffer.Bytes())
		callbackI, loaded := client.callbacks.LoadAndDelete(hdr.Sequence())
		if !loaded {
			continue
		}
		callback := callbackI.(*pingCallback)
		callback.data = hdr
		callback.conn.WriteBack(callback)
	}
	m.access.Lock()
	m.icmp4Conn = nil
	m.access.Unlock()
}

func (m *PingManager) loop6() {
	buffer := buf.StackNew()
	defer buffer.Release()
	for {
		buffer.Clear()
		_, err := buffer.ReadFromPacketConn(m.icmp6Conn)
		if err != nil {
			if err != os.ErrClosed {
				newError("icmp6 receive failed").Base(err)
				m.Close()
			}
			break
		}
		if buffer.Endpoint == nil {
			newError("nil icmp6 endpoint").WriteToLog()
			continue
		}
		client := m.getClient(*buffer.Endpoint)
		hdr := header.ICMPv6(buffer.Bytes())
		callbackI, loaded := client.callbacks.LoadAndDelete(hdr.Sequence())
		if !loaded {
			continue
		}
		callback := callbackI.(*pingCallback)
		callback.data = hdr
		callback.conn.WriteBack(callback)
	}
}

func (c *pingClient) nextSequence() uint16 {
	c.access.Lock()
	defer c.access.Unlock()
	c.sequence++
	if c.sequence == 0 {
		c.sequence++
	}
	return c.sequence
}

type pingClient struct {
	*PingManager
	ctx       context.Context
	timer     *signal.ActivityTimer
	dest      net.Destination
	access    sync.Mutex
	id        uint16
	sequence  uint16
	callbacks sync.Map
}

func (c *pingClient) createConnection() internet.Connection {
	ctx, cancel := context.WithCancel(c.ctx)
	base := &pingConnBase{pingClient: c, channel: make(chan *pingCallback), ctx: ctx, cancel: cancel}
	if c.dest.Address.Family().IsIPv4() {
		return &pingConn4{base}
	} else {
		return &pingConn6{base}
	}
}

type pingConnBase struct {
	*pingClient
	sync.Mutex
	ctx     context.Context
	cancel  context.CancelFunc
	channel chan *pingCallback
}

func (p *pingConnBase) WriteBack(callback *pingCallback) error {
	select {
	case <-p.ctx.Done():
		return io.ErrClosedPipe
	case p.channel <- callback:
		p.timer.Update()
	default:
	}
	return nil
}

func (p *pingConnBase) Close() error {
	select {
	case <-p.ctx.Done():
		return nil
	default:
	}
	p.Lock()
	defer p.Unlock()
	select {
	case <-p.ctx.Done():
		return nil
	default:
	}
	p.cancel()
	return nil
}

func (p *pingConnBase) LocalAddr() net.Addr {
	return &net.UnixAddr{}
}

func (p *pingConnBase) RemoteAddr() net.Addr {
	if p.unprivileged {
		return p.dest.UDPAddr()
	} else {
		return p.dest.IPAddr()
	}
}

func (p *pingConnBase) SetDeadline(t time.Time) error {
	return nil
}

func (p *pingConnBase) SetReadDeadline(t time.Time) error {
	return nil
}

func (p *pingConnBase) SetWriteDeadline(t time.Time) error {
	return nil
}

type pingCallback struct {
	conn     *pingConnBase
	id       uint16
	sequence uint16
	data     []byte
}

type pingConn4 struct {
	*pingConnBase
}

func (c *pingConn4) Read(b []byte) (n int, err error) {
	var callback *pingCallback
	select {
	case callback = <-c.channel:
		break
	case <-c.ctx.Done():
		return 0, io.ErrClosedPipe
	}
	hdr := header.ICMPv4(callback.data)
	sequence := hdr.Sequence()
	if sequence != callback.sequence {
		hdr.SetSequence(callback.sequence)
		hdr.SetChecksum(^header.ChecksumCombine(^hdr.Checksum(), header.ChecksumCombine(callback.sequence, ^sequence)))
	}
	newError("read ping request from ", c.dest.Address, " seq ", callback.sequence).AtDebug().WriteToLog()
	hdr.SetIdentWithChecksumUpdate(callback.id)
	return copy(b, hdr), nil
}

func (c *pingConn4) Write(b []byte) (n int, err error) {
	select {
	case <-c.ctx.Done():
		return 0, io.ErrClosedPipe
	default:
	}
	c.timer.Update()

	conn := c.icmp4Conn
	if conn == nil {
		return 0, io.ErrClosedPipe
	}
	sequence := c.nextSequence()
	hdr := header.ICMPv4(b)
	callback := pingCallback{
		conn:     c.pingConnBase,
		id:       hdr.Ident(),
		sequence: hdr.Sequence(),
	}
	c.callbacks.Store(sequence, &callback)
	if callback.sequence != sequence {
		hdr.SetSequence(sequence)
	}
	newError("write ping request to ", c.dest.Address, " seq ", sequence).AtDebug().WriteToLog()
	hdr.SetIdent(0)
	if !c.unprivileged {
		hdr.SetChecksum(0)
		hdr.SetChecksum(header.ICMPv4Checksum(hdr, 0))
	}
	return conn.WriteTo(hdr, c.RemoteAddr())
}

type pingConn6 struct {
	*pingConnBase
}

func (c *pingConn6) Read(b []byte) (n int, err error) {
	var callback *pingCallback
	select {
	case callback = <-c.channel:
		break
	case <-c.ctx.Done():
		return 0, io.ErrClosedPipe
	}
	hdr := header.ICMPv6(callback.data)
	if hdr.Sequence() != callback.sequence {
		hdr.SetSequence(callback.sequence)
	}
	newError("read ping request from ", c.dest.Address, " seq ", callback.sequence).AtDebug().WriteToLog()
	hdr.SetIdent(callback.id)
	return copy(b, hdr), nil
}

func (c *pingConn6) Write(b []byte) (n int, err error) {
	select {
	case <-c.ctx.Done():
		return 0, io.ErrClosedPipe
	default:
	}
	c.timer.Update()

	conn := c.icmp6Conn
	if conn == nil {
		return 0, io.ErrClosedPipe
	}
	sequence := c.nextSequence()
	hdr := header.ICMPv6(b)
	callback := pingCallback{
		conn:     c.pingConnBase,
		id:       hdr.Ident(),
		sequence: hdr.Sequence(),
	}
	c.callbacks.Store(sequence, &callback)
	if callback.sequence != sequence {
		hdr.SetSequence(sequence)
	}
	newError("write ping request to ", c.dest.Address, " seq ", sequence).AtDebug().WriteToLog()
	hdr.SetIdent(0)
	if !c.unprivileged {
		hdr.SetChecksum(0)
	}
	return conn.WriteTo(hdr, c.RemoteAddr())
}

func GetDestinationIsSubsetOf(dest net.Destination) bool {
	return dest.Network == net.Network_UDP && dest.Address.Family().IsIP() && dest.Port == 7
}
