package libcore

import (
	"context"
	"net"
	"net/netip"
	"strings"
	"syscall"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/dns/localdns"
	"golang.org/x/net/dns/dnsmessage"
	"libcore/comm"
)

type QueryContext struct {
	ctx     context.Context
	message []byte
	ips     []net.IP
	error   error
}

func (c *QueryContext) OnCancel(callback Func) {
	go func() {
		<-c.ctx.Done()
		callback.Invoke()
	}()
}

func (c *QueryContext) Success(result string) {
	c.ips = common.Map(common.Filter(strings.Split(result, "\n"), func(it string) bool {
		return common.IsNotBlank(it)
	}), func(it string) net.IP {
		return net.ParseIP(it)
	})
}

func (c *QueryContext) RawSuccess(result []byte) {
	c.message = make([]byte, len(result))
	copy(c.message, result)
}

func (c *QueryContext) ErrorCode(code int32) {
	c.error = dns.RCodeError(code)
}

func (c *QueryContext) Errno(errno int32) {
	c.error = syscall.Errno(errno)
}

type LocalResolver interface {
	HasRawSupport() bool
	QueryRaw(ctx *QueryContext, message []byte) error
	LookupIP(ctx *QueryContext, network string, domain string) error
}

var _ localdns.LocalTransport = (*localTransport)(nil)

type localTransport struct {
	r LocalResolver
}

func (l *localTransport) Type() dns.TransportType {
	if l.r.HasRawSupport() {
		return dns.TransportTypeExchangeRaw
	} else {
		return dns.TransportTypeLookup
	}
}

func (l *localTransport) Write(ctx context.Context, message *dnsmessage.Message) error {
	return common.ErrNoClue
}

func (l *localTransport) Exchange(ctx context.Context, message *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (l *localTransport) ExchangeRaw(ctx context.Context, message *buf.Buffer) (*buf.Buffer, error) {
	query := &QueryContext{
		ctx: ctx,
	}
	var response *buf.Buffer
	return response, task.Run(ctx, func() error {
		err := l.r.QueryRaw(query, message.Bytes())
		if err != nil {
			return err
		}
		if query.error != nil {
			return query.error
		}
		response = buf.FromBytes(query.message)
		return nil
	})
}

func (l *localTransport) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	var network string
	switch strategy {
	case dns.QueryStrategy_USE_IP4:
		network = "ip4"
	case dns.QueryStrategy_USE_IP6:
		network = "ip6"
	default:
		network = "ip"
	}
	query := &QueryContext{
		ctx: ctx,
	}
	var response []net.IP
	return response, task.Run(ctx, func() error {
		err := l.r.LookupIP(query, network, domain)
		if err != nil {
			return err
		}
		if query.error != nil {
			return query.error
		}
		response = query.ips
		if len(response) == 0 {
			return dns.ErrEmptyResponse
		}
		return nil
	})
}

func (l *localTransport) IsLocalTransport() {
}

func (l *localTransport) Close() error {
	return nil
}

func SetLocalhostResolver(local LocalResolver) {
	if local == nil {
		localdns.SetTransport(nil)
	} else {
		localdns.SetTransport(&localTransport{
			local,
		})
	}
}

func init() {
	SetCurrentDomainNameSystemQueryInstance(nil)
}

var dnsAddress = v2rayNet.IPAddress([]byte{1, 0, 0, 1})

func SetCurrentDomainNameSystemQueryInstance(instance *V2RayInstance) {
	if instance == nil {
		net.DefaultResolver = &net.Resolver{
			PreferGo: false,
		}
	} else {
		net.DefaultResolver = &net.Resolver{
			PreferGo: true,
			Dial: func(ctx context.Context, network, address string) (net.Conn, error) {
				conn, err := instance.dialContext(session.ContextWithInbound(ctx, &session.Inbound{
					Tag: "dns-in",
				}), v2rayNet.Destination{
					Network: v2rayNet.Network_UDP,
					Address: dnsAddress,
					Port:    53,
				})
				if err == nil {
					conn = &pinnedPacketConn{conn}
				}
				return conn, err
			},
		}
	}
}

type pinnedPacketConn struct {
	net.Conn
}

func (c *pinnedPacketConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	n, err = c.Conn.Read(p)
	if err == nil {
		addr = c.Conn.RemoteAddr()
	}
	return
}

func (c *pinnedPacketConn) WriteTo(p []byte, _ net.Addr) (n int, err error) {
	return c.Conn.Write(p)
}

func EncodeDomainNameSystemQuery(id int32, domain string, ipv6Mode int32) ([]byte, error) {
	if !strings.HasSuffix(domain, ".") {
		domain = domain + "."
	}
	name, err := dnsmessage.NewName(domain)
	if err != nil {
		return nil, newError("domain name too long").Base(err)
	}
	message := new(dnsmessage.Message)
	message.Header.ID = uint16(id)
	message.Header.RecursionDesired = true
	if ipv6Mode != comm.IPv6Only {
		message.Questions = append(message.Questions, dnsmessage.Question{
			Name:  name,
			Type:  dnsmessage.TypeA,
			Class: dnsmessage.ClassINET,
		})
	}
	if ipv6Mode != comm.IPv6Disable {
		message.Questions = append(message.Questions, dnsmessage.Question{
			Name:  name,
			Type:  dnsmessage.TypeAAAA,
			Class: dnsmessage.ClassINET,
		})
	}
	return message.Pack()
}

func DecodeContentDomainNameSystemResponse(content []byte) (response string, err error) {
	var (
		header       dnsmessage.Header
		answerHeader dnsmessage.ResourceHeader
		aAnswer      dnsmessage.AResource
		aaaaAnswer   dnsmessage.AAAAResource
	)
	parser := new(dnsmessage.Parser)
	if header, err = parser.Start(content); err != nil {
		err = newError("failed to parse DNS response").Base(err)
		return
	}
	if header.RCode != dnsmessage.RCodeSuccess {
		return "", newError("rcode: ", header.RCode.String())
	}
	if err = parser.SkipAllQuestions(); err != nil {
		err = newError("failed to skip questions in DNS response").Base(err)
		return
	}
	for {
		answerHeader, err = parser.AnswerHeader()
		if err != nil {
			if err != dnsmessage.ErrSectionDone {
				err = newError("failed to parse answer section for domain: ", answerHeader.Name.String()).Base(err)
			} else {
				err = nil
			}
			break
		}

		switch answerHeader.Type {
		case dnsmessage.TypeA:
			aAnswer, err = parser.AResource()
			if err != nil {
				err = newError("failed to parse A record for domain: ", answerHeader.Name).Base(err)
				return
			}
			response += " " + netip.AddrFrom4(aAnswer.A).String()
		case dnsmessage.TypeAAAA:
			aaaaAnswer, err = parser.AAAAResource()
			if err != nil {
				err = newError("failed to parse AAAA record for domain: ", answerHeader.Name).Base(err)
				return
			}
			response += " " + netip.AddrFrom16(aaaaAnswer.AAAA).String()
		default:
			if err = parser.SkipAnswer(); err != nil {
				err = newError("failed to skip answer").Base(err)
				return
			}
			continue
		}
	}
	return
}
