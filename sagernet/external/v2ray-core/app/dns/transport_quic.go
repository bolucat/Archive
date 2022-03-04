package dns

import (
	"context"
	"crypto/tls"
	"io"
	"sync"

	"github.com/lucas-clemente/quic-go"
	"golang.org/x/net/dns/dnsmessage"
	"golang.org/x/net/http2"
	"net/netip"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/retry"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

var _ dns.Transport = (*QUICTransport)(nil)

// NextProtoDQ - During connection establishment, DNS/QUIC support is indicated
// by selecting the ALPN token "dq" in the crypto handshake.
const NextProtoDQ = "doq-i00"

type QUICTransport struct {
	*transportContext
	dispatcher routing.Dispatcher

	access  sync.RWMutex
	session quic.Session
}

func (t *QUICTransport) Close() error {
	session := t.session
	if session != nil {
		session.CloseWithError(0, "")
	}
	return nil
}

func (t *QUICTransport) Type() dns.TransportType {
	return dns.TransportTypeExchange
}

func NewQUICTransport(ctx *transportContext, dispatcher routing.Dispatcher) *QUICTransport {
	return &QUICTransport{
		transportContext: ctx,
		dispatcher:       dispatcher,
	}
}

func NewQUICLocalTransport(ctx *transportContext) *QUICTransport {
	return &QUICTransport{
		transportContext: ctx,
	}
}

func (t *QUICTransport) getConnection(ctx context.Context) (quic.Session, error) {
	t.access.RLock()
	session := t.session
	t.access.RUnlock()

	if session != nil && !common.Done(session.Context()) {
		return session, nil
	}

	t.access.Lock()
	defer t.access.Unlock()

	var destinations []net.Destination
	domain := t.destination.Address.String()
	addr, err := netip.ParseAddr(domain)
	if err != nil {
		ips, _, err := t.client.LookupDefault(ctx, domain)
		if err != nil {
			return nil, newError("failed to lookup server address").Base(err)
		}
		destinations = common.Map(ips, func(it net.IP) net.Destination {
			destination := t.destination
			destination.Address = net.IPAddress(it)
			return destination
		})
	} else {
		destination := t.destination
		destination.Address = net.IPAddress(addr.AsSlice())
		destinations = []net.Destination{destination}
	}

	index := -1
	err = retry.ExponentialBackoff(len(destinations), 0).On(func() error {
		index++
		destination := destinations[index]
		var packetConn net.PacketConn
		if t.dispatcher != nil {
			link, err := t.dispatcher.Dispatch(t.newContext(), destination)
			if err != nil {
				return err
			}
			packetConn = &pinnedPacketConn{
				buf.NewConnection(buf.ConnectionInputMulti(link.Writer), buf.ConnectionOutputMulti(link.Reader)),
				destination.UDPAddr(),
			}
		} else {
			conn, err := internet.ListenSystemPacket(t.newContext(), &net.UDPAddr{IP: net.AnyIP.IP(), Port: 0}, nil)
			if err != nil {
				return err
			}
			packetConn = conn
		}

		tlsConfig := &tls.Config{
			NextProtos: []string{"http/1.1", http2.NextProtoTLS, NextProtoDQ},
		}

		quicSession, err := quic.DialEarlyContext(t.ctx, packetConn, destination.UDPAddr(), domain, tlsConfig, nil)
		if err != nil {
			return err
		}
		session = quicSession
		return nil
	})

	if err != nil {
		return nil, err
	}

	t.session = session
	return session, nil
}

func (t *QUICTransport) Exchange(ctx context.Context, message *dnsmessage.Message) (*dnsmessage.Message, error) {
	session, err := t.getConnection(ctx)
	if err != nil {
		return nil, err
	}

	requestId := message.ID
	message.ID = 0

	stream, err := session.OpenStreamSync(ctx)
	if err != nil {
		return nil, err
	}

	var response *dnsmessage.Message
	return response, task.Run(ctx, func() error {
		packed, err := message.Pack()
		if err != nil {
			return err
		}

		_, err = stream.Write(packed)
		if err != nil {
			return err
		}

		buffer := buf.New()
		n, err := stream.Read(buffer.Extend(buf.Size))
		if err != nil && err != io.EOF {
			buffer.Release()
			return err
		}
		buffer.Resize(0, int32(n))
		stream.Close()

		response = new(dnsmessage.Message)
		err = response.Unpack(buffer.Bytes())
		if err != nil {
			return err
		}
		response.ID = requestId
		return nil
	})
}

func (t *QUICTransport) Write(context.Context, *dnsmessage.Message) error {
	return common.ErrNoClue
}

func (t *QUICTransport) ExchangeRaw(context.Context, *buf.Buffer) (*buf.Buffer, error) {
	return nil, common.ErrNoClue
}

func (t *QUICTransport) Lookup(context.Context, string, dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
