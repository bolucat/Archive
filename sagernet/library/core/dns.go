package libcore

import (
	"context"
	"net"
	"strings"

	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"golang.org/x/net/dns/dnsmessage"
	"libcore/comm"
	"net/netip"
)

func init() {
	SetCurrentDomainNameSystemQueryInstance(nil)
}

func SetCurrentDomainNameSystemQueryInstance(instance *V2RayInstance) {
	if instance == nil {
		net.DefaultResolver = &net.Resolver{
			PreferGo: false,
		}
	} else {
		dnsAdress := v2rayNet.IPAddress([]byte{1, 0, 0, 1})
		net.DefaultResolver = &net.Resolver{
			Dial: func(ctx context.Context, network, address string) (net.Conn, error) {
				conn, err := instance.dialContext(session.ContextWithInbound(ctx, &session.Inbound{
					Tag: "dns-in",
				}), v2rayNet.Destination{
					Network: v2rayNet.Network_UDP,
					Address: dnsAdress,
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
