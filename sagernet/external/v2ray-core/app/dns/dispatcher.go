package dns

import (
	"context"
	"crypto/tls"
	"io"
	"sync"
	time "time"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/common/signal"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

type (
	writeBackFunc func(message *buf.Buffer)
	dialerFunc    func() (net.Conn, error)
	convertFunc   func(conn net.Conn) (net.Conn, error)
)

type messageDispatcher struct {
	dialer      dialerFunc
	destination net.Destination
	writeBack   writeBackFunc

	access     sync.Mutex
	connection *dispatcherConnection
}

func (d *messageDispatcher) Close() error {
	connection := d.connection
	if connection != nil {
		connection.cancel()
		common.Interrupt(connection.link.Reader)
		common.Interrupt(connection.link.Writer)
		d.connection = nil
	}
	return nil
}

func NewDispatcher(ctx *transportContext, dispatcher routing.Dispatcher, destination net.Destination, writeBack writeBackFunc) *messageDispatcher {
	return &messageDispatcher{
		dialer: func() (net.Conn, error) {
			link, err := dispatcher.Dispatch(ctx.newContext(), destination)
			common.Must(err)
			return buf.NewConnection(buf.ConnectionInputMulti(link.Writer), buf.ConnectionOutputMulti(link.Reader)), nil
		},
		destination: destination,
		writeBack:   writeBack,
	}
}

func NewLocalDispatcher(ctx *transportContext, destination net.Destination, writeBack writeBackFunc) *messageDispatcher {
	return &messageDispatcher{
		dialer: func() (net.Conn, error) {
			var sockopt internet.SocketConfig
			if destination.Network == net.Network_TCP {
				sockopt.Tfo = internet.SocketConfig_Enable
				sockopt.TcpKeepAliveInterval = 15
			}
			return internet.DialSystemDNS(ctx.newContext(), destination, &sockopt)
		},
		destination: destination,
		writeBack:   writeBack,
	}
}

func NewRawDispatcher(dialer dialerFunc, destination net.Destination, writeBack writeBackFunc) *messageDispatcher {
	return &messageDispatcher{
		dialer:      dialer,
		destination: destination,
		writeBack:   writeBack,
	}
}

func NewRawLocalDispatcher(ctx *transportContext, convertor convertFunc, destination net.Destination, writeBack writeBackFunc) *messageDispatcher {
	return &messageDispatcher{
		dialer: func() (net.Conn, error) {
			var sockopt internet.SocketConfig
			if destination.Network == net.Network_TCP {
				sockopt.Tfo = internet.SocketConfig_Enable
				sockopt.TcpKeepAliveInterval = 15
			}
			conn, err := internet.DialSystemDNS(ctx.newContext(), destination, &sockopt)
			if err != nil {
				return nil, err
			}
			return convertor(conn)
		},
		destination: destination,
		writeBack:   writeBack,
	}
}

func (d *messageDispatcher) Write(ctx context.Context, message *buf.Buffer) error {
	conn, err := d.getConnection()
	if err != nil {
		return err
	}
	if conn.tlsConn != nil {
		err = conn.tlsConn.HandshakeContext(ctx)
		if err != nil {
			return err
		}
	}
	outputStream := conn.link.Writer
	if outputStream == nil {
		conn.cancel()
		return io.ErrClosedPipe
	}
	return outputStream.WriteMultiBuffer(buf.MultiBuffer{message})
}

func (d *messageDispatcher) getConnection() (*dispatcherConnection, error) {
	d.access.Lock()
	defer d.access.Unlock()

	if d.connection != nil && !common.Done(d.connection.ctx) {
		return d.connection, nil
	}

	ctx, cancel := context.WithCancel(context.Background())
	timer := signal.CancelAfterInactivity(ctx, cancel, 5*time.Minute)
	newError("establishing new connection for ", d.destination).WriteToLog()
	link, err := d.dialer()
	if err != nil {
		return nil, err
	}
	conn := &dispatcherConnection{
		link: &transport.Link{
			Reader: buf.NewReader(link),
			Writer: buf.NewWriter(link),
		},
		ctx:       ctx,
		cancel:    cancel,
		timer:     timer,
		writeBack: d.writeBack,
	}
	if tlsConn, ok := link.(*tls.Conn); ok {
		conn.tlsConn = tlsConn
	}
	d.connection = conn
	go conn.handleInput()
	return conn, nil
}

type dispatcherConnection struct {
	tlsConn   *tls.Conn
	ctx       context.Context
	link      *transport.Link
	timer     signal.ActivityUpdater
	cancel    context.CancelFunc
	writeBack writeBackFunc
}

func (c *dispatcherConnection) handleInput() {
	defer c.cancel()

	input := c.link.Reader
	timer := c.timer

	for {
		if common.Done(c.ctx) {
			return
		}

		mb, err := input.ReadMultiBuffer()
		if err != nil {
			if err != io.EOF {
				newError("dns connection closed").Base(err).WriteToLog(session.ExportIDToError(c.ctx))
			}
			return
		}
		timer.Update()
		for _, b := range mb {
			c.writeBack(b)
		}
	}
}

type pinnedPacketConn struct {
	net.Conn
	addr net.Addr
}

func (c *pinnedPacketConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	n, err = c.Read(p)
	if err == nil {
		addr = c.addr
	}
	return
}

func (c *pinnedPacketConn) WriteTo(p []byte, _ net.Addr) (n int, err error) {
	return c.Write(p)
}
