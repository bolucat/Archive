package shadowsocks_sing

import (
	"context"
	"io"
	"runtime"
	"time"

	C "github.com/sagernet/sing/common"
	B "github.com/sagernet/sing/common/buf"
	E "github.com/sagernet/sing/common/exceptions"
	M "github.com/sagernet/sing/common/metadata"
	N "github.com/sagernet/sing/common/network"
	"github.com/sagernet/sing/common/random"
	"github.com/sagernet/sing/common/rw"
	"github.com/sagernet/sing/protocol/shadowsocks"
	"github.com/sagernet/sing/protocol/shadowsocks/shadowimpl"
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
	var rng io.Reader = random.Default
	if config.ReducedIvHeadEntropy {
		rng = &shadowsocks.ReducedEntropyReader{
			Reader: rng,
		}
	}
	method, err := shadowimpl.FetchMethod(config.Method, config.Key, config.Password, rng)
	if err != nil {
		return nil, err
	}
	o.method = method
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
	/*if statConn, ok := inboundConn.(*internet.StatCounterConn); ok {
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
		serverConn := o.method.DialEarlyConn(connection, SingDestination(destination))

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
				runtime.KeepAlive(_payload)
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
				runtime.KeepAlive(_payload)
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

		conn := &PipeConnWrapper{
			W:       link.Writer,
			PipeOut: pipeOut,
			Conn:    inboundConn,
		}
		if ir, ok := link.Reader.(io.Reader); ok {
			conn.R = ir
		} else {
			conn.R = &buf.BufferedReader{Reader: link.Reader}
		}

		return rw.CopyConn(ctx, conn, serverConn)
	} else {
		var packetConn N.PacketConn
		if pc, isPacketConn := inboundConn.(N.PacketConn); isPacketConn {
			packetConn = pc
		} else if nc, isNetPacket := inboundConn.(net.PacketConn); isNetPacket {
			packetConn = &N.PacketConnWrapper{PacketConn: nc}
		} else {
			packetConn = &PacketConnWrapper{
				Reader:  link.Reader,
				Writer:  link.Writer,
				PipeOut: pipe.IsPipe(link.Writer),
				Conn:    inboundConn,
				Dest:    destination,
			}
		}

		serverConn := o.method.DialPacketConn(connection)
		return N.CopyPacketConn(ctx, packetConn, serverConn)
	}
}

func (o *Outbound) ProcessConn(ctx context.Context, conn net.Conn, dialer internet.Dialer) error {
	outbound := session.OutboundFromContext(ctx)
	if outbound == nil || !outbound.Target.IsValid() {
		return newError("target not specified")
	}
	/*if statConn, ok := inboundConn.(*internet.StatCounterConn); ok {
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

	serverConn := o.method.DialEarlyConn(connection, SingDestination(destination))

	if cr, ok := conn.(rw.CachedReader); ok {
		cached := cr.ReadCached()
		if cached != nil && !cached.IsEmpty() {
			_, err = serverConn.Write(cached.Bytes())
			cached.Release()
			if err != nil {
				return newError("client handshake").Base(err)
			}
			goto direct
		}
	}

	{
		err = conn.SetReadDeadline(time.Now().Add(100 * time.Millisecond))
		if err != nil {
			return err
		}

		_request := B.StackNew()
		request := C.Dup(_request)

		_, err = request.ReadFrom(conn)
		if err != nil && !E.IsTimeout(err) {
			return err
		}

		err = conn.SetReadDeadline(time.Time{})
		if err != nil {
			return err
		}

		_, err = serverConn.Write(request.Bytes())
		if err != nil {
			return newError("client handshake").Base(err)
		}
		runtime.KeepAlive(_request)
	}

direct:
	return rw.CopyConn(ctx, conn, serverConn)
}

func SingDestination(destination net.Destination) M.Socksaddr {
	var addr M.Socksaddr
	switch destination.Address.Family() {
	case net.AddressFamilyDomain:
		addr.Fqdn = destination.Address.Domain()
	default:
		addr.Addr = M.AddrFromIP(destination.Address.IP())
	}
	addr.Port = uint16(destination.Port)
	return addr
}

type PipeConnWrapper struct {
	R       io.Reader
	W       buf.Writer
	PipeOut bool
	net.Conn
}

func (w *PipeConnWrapper) Close() error {
	common.Interrupt(w.R)
	common.Interrupt(w.W)
	common.Close(w.Conn)
	return nil
}

func (w *PipeConnWrapper) Read(b []byte) (n int, err error) {
	return w.R.Read(b)
}

func (w *PipeConnWrapper) Write(p []byte) (n int, err error) {
	if w.PipeOut {
		// avoid bad usage of stack buffer
		buffer := buf.New()
		_, err = buffer.Write(p)
		if err != nil {
			buffer.Release()
			return
		}
		err = w.W.WriteMultiBuffer(buf.MultiBuffer{buffer})
		if err != nil {
			buffer.Release()
			return
		}
	} else {
		err = w.W.WriteMultiBuffer(buf.MultiBuffer{buf.FromBytes(p)})
		if err != nil {
			return
		}
	}
	n = len(p)
	return
}

type PacketConnWrapper struct {
	buf.Reader
	buf.Writer
	net.Conn
	PipeOut bool
	Dest    net.Destination
	cached  buf.MultiBuffer
}

func (w *PacketConnWrapper) ReadPacket(buffer *B.Buffer) (M.Socksaddr, error) {
	if w.cached != nil {
		mb, bb := buf.SplitFirst(w.cached)
		if bb == nil {
			w.cached = nil
		} else {
			buffer.Write(bb.Bytes())
			w.cached = mb
			var destination net.Destination
			if bb.Endpoint != nil {
				destination = *bb.Endpoint
			} else {
				destination = w.Dest
			}
			bb.Release()
			return SingDestination(destination), nil
		}
	}
	mb, err := w.ReadMultiBuffer()
	if err != nil {
		return M.Socksaddr{}, err
	}
	nb, bb := buf.SplitFirst(mb)
	if bb == nil {
		return M.Socksaddr{}, nil
	} else {
		buffer.Write(bb.Bytes())
		w.cached = nb
		var destination net.Destination
		if bb.Endpoint != nil {
			destination = *bb.Endpoint
		} else {
			destination = w.Dest
		}
		bb.Release()
		return SingDestination(destination), nil
	}
}

func (w *PacketConnWrapper) WritePacket(buffer *B.Buffer, addrPort M.Socksaddr) error {
	var vBuf *buf.Buffer
	if w.PipeOut {
		vBuf = buf.New()
		vBuf.Write(buffer.Bytes())
	} else {
		vBuf = buf.FromBytes(buffer.Bytes())
	}
	endpoint := net.DestinationFromAddr(addrPort.UDPAddr())
	vBuf.Endpoint = &endpoint
	return w.Writer.WriteMultiBuffer(buf.MultiBuffer{vBuf})
}

func (w *PacketConnWrapper) Close() error {
	common.Interrupt(w.Reader)
	common.Close(w.Conn)
	buf.ReleaseMulti(w.cached)
	return nil
}
