package shadowsocks_sing

import (
	"context"
	"encoding/base64"
	"io"
	"strings"
	"time"

	C "github.com/sagernet/sing/common"
	B "github.com/sagernet/sing/common/buf"
	M "github.com/sagernet/sing/common/metadata"
	"github.com/sagernet/sing/common/random"
	"github.com/sagernet/sing/common/rw"
	"github.com/sagernet/sing/protocol/shadowsocks"
	"github.com/sagernet/sing/protocol/shadowsocks/shadowaead"
	"github.com/sagernet/sing/protocol/shadowsocks/shadowaead_2022"
	"github.com/sagernet/sing/protocol/socks"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
	"github.com/v2fly/v2ray-core/v5/transport/pipe"
)

func init() {
	common.Must(common.RegisterConfig((*ClientConfig)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		return NewClient(ctx, config.(*ClientConfig))
	}))
}

type Outbound struct {
	ctx    context.Context
	server net.Destination
	method shadowsocks.Method
}

func NewClient(ctx context.Context, config *ClientConfig) (*Outbound, error) {
	o := &Outbound{
		ctx: ctx,
		server: net.Destination{
			Address: config.Address.AsAddress(),
			Port:    net.Port(config.Port),
			Network: net.Network_TCP,
		},
	}
	if config.Method == shadowsocks.MethodNone {
		o.method = shadowsocks.NewNone()
	} else if common.Contains(shadowaead.List, config.Method) {
		var key []byte
		if config.Key != "" {
			bKdy, err := base64.StdEncoding.DecodeString(config.Key)
			if err != nil {
				return nil, newError("shadowsocks: decode key ", config.Key).Base(err)
			}
			key = bKdy
		}
		rng := random.Blake3KeyedHash()
		if config.ReducedIvHeadEntropy {
			rng = &shadowsocks.ReducedEntropyReader{
				Reader: rng,
			}
		}
		method, err := shadowaead.New(config.Method, key, []byte(config.Password), rng, false)
		if err != nil {
			return nil, newError("create method").Base(err)
		}
		o.method = method
	} else if common.Contains(shadowaead_2022.List, config.Method) {
		if config.Password != "" {
			return nil, newError("use psk instead of password")
		}
		if config.Key == "" {
			return nil, newError("missing psk")
		}
		var pskList [][]byte
		for _, psk := range strings.Split(config.Key, ":") {
			bKdy, err := base64.StdEncoding.DecodeString(psk)
			if err != nil {
				return nil, newError("decode key ", psk).Base(err)
			}
			pskList = append(pskList, bKdy)
		}
		rng := random.Blake3KeyedHash()
		if config.ReducedIvHeadEntropy {
			rng = &shadowsocks.ReducedEntropyReader{
				Reader: rng,
			}
		}
		method, err := shadowaead_2022.New(config.Method, pskList, rng)
		if err != nil {
			return nil, newError("create method").Base(err)
		}
		o.method = method
	} else {
		return nil, newError("unknown method ", config.Method)
	}
	return o, nil
}

func (o *Outbound) Process(ctx context.Context, link *transport.Link, dialer internet.Dialer) error {
	var inboundConn net.Conn
	inbound := session.InboundFromContext(ctx)
	if inbound != nil {
		inboundConn = inbound.Conn
	}

	outbound := session.OutboundFromContext(ctx)
	if outbound == nil || !outbound.Target.IsValid() {
		return newError("target not specified")
	}
	/*if statConn, ok := inboundConn.(*internet.StatCouterConnection); ok {
		inboundConn = statConn.Connection
	}*/
	destination := outbound.Target
	network := destination.Network

	newError("tunneling request to ", destination, " via ", o.server.NetAddr()).WriteToLog(session.ExportIDToError(ctx))

	serverDestination := o.server
	serverDestination.Network = network
	connection, err := dialer.Dial(ctx, serverDestination)
	if err != nil {
		return newError("failed to connect to server").Base(err)
	}

	connElem := net.AddConnection(connection)
	defer net.RemoveConnection(connElem)

	if network == net.Network_TCP {
		serverConn := o.method.DialEarlyConn(connection, singDestination(destination))

		var handshake bool
		if cachedReader, isCached := link.Reader.(pipe.CachedReader); isCached {
			cached, _ := cachedReader.ReadMultiBufferCached()
			if cached != nil && !cached.IsEmpty() {
				_payload := B.StackNew()
				payload := C.Dup(_payload)
				for {
					payload.FullReset()
					nb, n := buf.SplitBytes(cached, payload.FreeBytes())
					if n > 0 {
						payload.Truncate(n)
						_, err = serverConn.Write(payload.Bytes())
						if err != nil {
							return newError("write payload").Base(err)
						}
						handshake = true
					}
					if nb.IsEmpty() {
						break
					} else {
						cached = nb
					}
				}
			}
		}
		if !handshake {
			if timeoutReader, isTimeoutReader := link.Reader.(buf.TimeoutReader); isTimeoutReader {
				mb, err := timeoutReader.ReadMultiBufferTimeout(time.Millisecond * 100)
				if err != nil && err != buf.ErrNotTimeoutReader && err != buf.ErrReadTimeout {
					return newError("read payload").Base(err)
				}
				_payload := B.StackNew()
				payload := C.Dup(_payload)
				for {
					payload.FullReset()
					nb, n := buf.SplitBytes(mb, payload.FreeBytes())
					if n > 0 {
						payload.Truncate(n)
						_, err = serverConn.Write(payload.Bytes())
						if err != nil {
							return newError("write payload").Base(err)
						}
						handshake = true
					}
					if nb.IsEmpty() {
						break
					} else {
						mb = nb
					}
				}
			}
		}
		if !handshake {
			_, err = serverConn.Write(nil)
			if err != nil {
				return newError("client handshake").Base(err)
			}
		}

		pipeIn := pipe.IsPipe(link.Reader)
		pipeOut := pipe.IsPipe(link.Writer)

		if inboundConn != nil && !pipeIn && !pipeOut {
			return rw.CopyConn(ctx, inboundConn, serverConn)
		}

		conn := &pipeConnWrapper{
			w:       link.Writer,
			pipeOut: pipeOut,
			Conn:    inboundConn,
		}
		if ir, ok := link.Reader.(io.Reader); ok {
			conn.r = ir
		} else {
			conn.r = &buf.BufferedReader{Reader: link.Reader}
		}

		return rw.CopyConn(ctx, conn, serverConn)
	} else {
		var packetConn socks.PacketConn
		if pc, isPacketConn := inboundConn.(socks.PacketConn); isPacketConn {
			packetConn = pc
		} else {
			packetConn = &packetConnWrapper{
				Reader: link.Reader,
				Writer: link.Writer,
				Conn:   inboundConn,
				dest:   destination,
			}
		}

		serverConn := o.method.DialPacketConn(connection)
		return socks.CopyPacketConn(ctx, packetConn, serverConn)
	}
}

func singDestination(destination net.Destination) *M.AddrPort {
	var addr M.Addr
	switch destination.Address.Family() {
	case net.AddressFamilyDomain:
		addr = M.AddrFromFqdn(destination.Address.Domain())
	default:
		addr = M.AddrFromIP(destination.Address.IP())
	}
	return M.AddrPortFrom(addr, uint16(destination.Port))
}

type pipeConnWrapper struct {
	r       io.Reader
	w       buf.Writer
	pipeOut bool
	net.Conn
}

func (w *pipeConnWrapper) Close() error {
	common.Interrupt(w.r)
	common.Interrupt(w.w)
	common.Close(w.Conn)
	return nil
}

func (w *pipeConnWrapper) Read(b []byte) (n int, err error) {
	return w.r.Read(b)
}

func (w *pipeConnWrapper) Write(p []byte) (n int, err error) {
	if w.pipeOut {
		// avoid bad usage of stack buffer
		buffer := buf.New()
		_, err = buffer.Write(p)
		if err != nil {
			return
		}
		err = w.w.WriteMultiBuffer(buf.MultiBuffer{buffer})
		if err != nil {
			buffer.Release()
			return
		}
	} else {
		err = w.w.WriteMultiBuffer(buf.MultiBuffer{buf.FromBytes(p)})
		if err != nil {
			return
		}
	}
	n = len(p)
	return
}

type packetConnWrapper struct {
	buf.Reader
	buf.Writer
	net.Conn
	dest   net.Destination
	cached buf.MultiBuffer
}

func (c *packetConnWrapper) ReadPacket(buffer *B.Buffer) (*M.AddrPort, error) {
	if c.cached != nil {
		mb, bb := buf.SplitFirst(c.cached)
		if bb == nil {
			c.cached = nil
		} else {
			buffer.Write(bb.Bytes())
			bb.Release()
			c.cached = mb
			var destination net.Destination
			if bb.Endpoint != nil {
				destination = *bb.Endpoint
			} else {
				destination = c.dest
			}
			return singDestination(destination), nil
		}
	}
	mb, err := c.ReadMultiBuffer()
	if err != nil {
		return nil, err
	}
	nb, bb := buf.SplitFirst(mb)
	if bb == nil {
		return nil, nil
	} else {
		buffer.Write(bb.Bytes())
		bb.Release()
		c.cached = nb
		var destination net.Destination
		if bb.Endpoint != nil {
			destination = *bb.Endpoint
		} else {
			destination = c.dest
		}
		return singDestination(destination), nil
	}
}

func (c *packetConnWrapper) WritePacket(buffer *B.Buffer, addrPort *M.AddrPort) error {
	vBuf := buf.FromBytes(buffer.Bytes())
	endpoint := net.DestinationFromAddr(addrPort.UDPAddr())
	vBuf.Endpoint = &endpoint
	return c.Writer.WriteMultiBuffer(buf.MultiBuffer{vBuf})
}
