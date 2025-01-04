package outbounds

import (
	"net"
	"strconv"

	"github.com/apernet/hysteria/core/v2/server"
)

// The PluggableOutbound system is designed to function in a chain-like manner.
// Not every outbound is an actual outbound; some are just wrappers around other
// outbounds, such as custom resolvers, ACL engine, etc. It is a pipeline where
// each stage can check (and optionally modify) the request before passing it
// on to the next stage. The last stage in the pipeline is always a real outbound
// that actually implements the logic of connecting to the remote server.
// There can also be instances of branching, where requests can be sent to
// different outbound sub-pipelines based on some criteria.

// PluggableOutbound differs from the built-in Outbound interface from Hysteria core
// in that it uses an AddrEx struct for addresses instead of a string. Because of this
// difference, we need a special PluggableOutboundAdapter to convert between the two
// for use in Hysteria core config.
type PluggableOutbound interface {
	TCP(reqAddr *AddrEx) (net.Conn, error)
	UDP(reqAddr *AddrEx) (UDPConn, error)
}

type UDPConn interface {
	ReadFrom(b []byte) (int, *AddrEx, error)
	WriteTo(b []byte, addr *AddrEx) (int, error)
	Close() error
}

// AddrEx keeps both the original string representation of the address and
// the resolved IP addresses from the resolver, if any.
// The actual outbound implementations can choose to use either the string
// representation or the resolved IP addresses, depending on their capabilities.
// A SOCKS5 outbound, for example, should prefer the string representation
// because SOCKS5 protocol supports sending the hostname to the proxy server
// and let the proxy server do the DNS resolution.
type AddrEx struct {
	Host        string // String representation of the host, can be an IP or a domain name
	Port        uint16
	ResolveInfo *ResolveInfo // Only set if there's a resolver in the pipeline
}

func (a *AddrEx) String() string {
	return net.JoinHostPort(a.Host, strconv.Itoa(int(a.Port)))
}

// ResolveInfo contains the resolved IP addresses from the resolver, and any
// error that occurred during the resolution.
// Note that there could be no error but also no resolved IP addresses,
// or there could be an error but also some resolved IP addresses.
// It's up to the actual outbound implementation to decide how to handle
// these cases.
type ResolveInfo struct {
	IPv4 net.IP
	IPv6 net.IP
	Err  error
}

var _ server.Outbound = (*PluggableOutboundAdapter)(nil)

type PluggableOutboundAdapter struct {
	PluggableOutbound
}

func (a *PluggableOutboundAdapter) TCP(reqAddr string) (net.Conn, error) {
	host, port, err := net.SplitHostPort(reqAddr)
	if err != nil {
		return nil, err
	}
	portInt, err := strconv.Atoi(port)
	if err != nil {
		return nil, err
	}
	return a.PluggableOutbound.TCP(&AddrEx{
		Host: host,
		Port: uint16(portInt),
	})
}

func (a *PluggableOutboundAdapter) UDP(reqAddr string) (server.UDPConn, error) {
	host, port, err := net.SplitHostPort(reqAddr)
	if err != nil {
		return nil, err
	}
	portInt, err := strconv.Atoi(port)
	if err != nil {
		return nil, err
	}
	conn, err := a.PluggableOutbound.UDP(&AddrEx{
		Host: host,
		Port: uint16(portInt),
	})
	if err != nil {
		return nil, err
	}
	return &udpConnAdapter{conn}, nil
}

type udpConnAdapter struct {
	UDPConn
}

func (u *udpConnAdapter) ReadFrom(b []byte) (int, string, error) {
	n, addr, err := u.UDPConn.ReadFrom(b)
	if addr != nil {
		return n, addr.String(), err
	} else {
		return n, "", err
	}
}

func (u *udpConnAdapter) WriteTo(b []byte, addr string) (int, error) {
	host, port, err := net.SplitHostPort(addr)
	if err != nil {
		return 0, err
	}
	portInt, err := strconv.Atoi(port)
	if err != nil {
		return 0, err
	}
	return u.UDPConn.WriteTo(b, &AddrEx{
		Host: host,
		Port: uint16(portInt),
	})
}

func (u *udpConnAdapter) Close() error {
	return u.UDPConn.Close()
}
