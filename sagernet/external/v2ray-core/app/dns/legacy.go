package dns

import (
	"context"
	"encoding/binary"
	"fmt"
	"net/url"
	"strings"

	"golang.org/x/net/dns/dnsmessage"

	core "github.com/v2fly/v2ray-core/v5"
	"github.com/v2fly/v2ray-core/v5/app/router"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/errors"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/platform"
	"github.com/v2fly/v2ray-core/v5/common/strmatcher"
	"github.com/v2fly/v2ray-core/v5/common/uuid"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/dns/localdns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/infra/conf/cfgcommon"
	"github.com/v2fly/v2ray-core/v5/infra/conf/geodata"
)

var ErrExpectedIPNonMatch = errors.New("expectIPs not match")

func init() {
	common.Must(common.RegisterConfig((*SimplifiedConfig)(nil), func(ctx context.Context, config interface{}) (interface{}, error) { // nolint: staticcheck
		ctx = cfgcommon.NewConfigureLoadingContext(context.Background()) // nolint: staticcheck

		geoloadername := platform.NewEnvFlag("v2ray.conf.geoloader").GetValue(func() string {
			return "standard"
		})

		if loader, err := geodata.GetGeoDataLoader(geoloadername); err == nil {
			cfgcommon.SetGeoDataLoader(ctx, loader)
		} else {
			return nil, newError("unable to create geo data loader ").Base(err)
		}

		cfgEnv := cfgcommon.GetConfigureLoadingEnvironment(ctx)
		geoLoader := cfgEnv.GetGeoLoader()

		simplifiedConfig := config.(*SimplifiedConfig)
		for _, v := range simplifiedConfig.NameServer {
			for _, geo := range v.Geoip {
				if geo.Code != "" {
					filepath := "geoip.dat"
					if geo.FilePath != "" {
						filepath = geo.FilePath
					} else {
						geo.CountryCode = geo.Code
					}
					var err error
					geo.Cidr, err = geoLoader.LoadIP(filepath, geo.Code)
					if err != nil {
						return nil, newError("unable to load geoip").Base(err)
					}
				}
			}
		}

		var nameservers []*NameServer

		for _, v := range simplifiedConfig.NameServer {
			nameserver := &NameServer{
				Address:      v.Address,
				ClientIp:     net.ParseIP(v.ClientIp),
				SkipFallback: v.SkipFallback,
				Geoip:        v.Geoip,
				Concurrency:  v.Concurrency,
			}
			for _, prioritizedDomain := range v.PrioritizedDomain {
				nameserver.PrioritizedDomain = append(nameserver.PrioritizedDomain, &NameServer_PriorityDomain{
					Type:   prioritizedDomain.Type,
					Domain: prioritizedDomain.Domain,
				})
			}
			nameservers = append(nameservers, nameserver)
		}

		fullConfig := &Config{
			NameServer:      nameservers,
			ClientIp:        net.ParseIP(simplifiedConfig.ClientIp),
			StaticHosts:     simplifiedConfig.StaticHosts,
			Tag:             simplifiedConfig.Tag,
			DisableCache:    simplifiedConfig.DisableCache,
			QueryStrategy:   simplifiedConfig.QueryStrategy,
			DisableFallback: simplifiedConfig.DisableFallback,
		}
		return common.CreateObject(ctx, fullConfig)
	}))
}

func init() {
	common.Must(common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		return New(ctx, config.(*Config))
	}))
}

func New(ctx context.Context, config *Config) (*Client, error) {
	var tag string
	if len(config.Tag) > 0 {
		tag = config.Tag
	} else {
		tag = GenerateRandomTag()
	}

	var clientIP net.IP
	switch len(config.ClientIp) {
	case 0, net.IPv4len, net.IPv6len:
		clientIP = net.IP(config.ClientIp)
	default:
		return nil, newError("unexpected client IP length ", len(config.ClientIp))
	}

	hosts, err := NewStaticHosts(config.StaticHosts, config.Hosts)
	if err != nil {
		return nil, newError("failed to create hosts").Base(err)
	}

	var servers []*Server
	domainRuleCount := 0
	for _, ns := range config.NameServer {
		domainRuleCount += len(ns.PrioritizedDomain)
	}

	matcherInfos := make([]DomainMatcherInfo, domainRuleCount+1)
	domainMatcher := strmatcher.NewMixedIndexMatcher()
	geoipContainer := router.GeoIPMatcherContainer{}

	core := core.MustFromContext(ctx)
	dispatcher, _ := core.GetFeature(routing.DispatcherType()).(routing.Dispatcher)

	client := &Client{
		ctx:      ctx,
		tag:      tag,
		clientIP: clientIP,

		defaultQueryStrategy: dns.QueryStrategy(config.QueryStrategy),

		hosts:         hosts,
		servers:       servers,
		domainMatcher: domainMatcher,
		matcherInfos:  matcherInfos,

		disableCache:           config.DisableCache,
		disableFallback:        config.DisableFallback,
		disableFallbackIfMatch: config.DisableFallbackIfMatch,
		disableExpire:          config.DisableExpire,
	}

	for _, ns := range config.NameServer {
		clientIdx := len(servers)
		updateDomain := func(domainRule strmatcher.Matcher, originalRuleIdx int, matcherInfos []DomainMatcherInfo) error {
			midx := domainMatcher.Add(domainRule)
			matcherInfos[midx] = DomainMatcherInfo{
				ClientIdx:     uint16(clientIdx),
				DomainRuleIdx: uint16(originalRuleIdx),
			}
			return nil
		}

		myClientIP := clientIP
		switch len(ns.ClientIp) {
		case net.IPv4len, net.IPv6len:
			myClientIP = net.IP(ns.ClientIp)
		}
		server, err := newServer(ctx, client, dispatcher, ns, myClientIP, geoipContainer, &matcherInfos, updateDomain)
		if err != nil {
			return nil, newError("failed to create client").Base(err)
		}
		servers = append(servers, server)
	}

	if len(servers) == 0 {
		servers = append(servers, &Server{
			name:      "localhost",
			transport: localdns.Transport(),
		})
	}

	err = domainMatcher.Build()
	if err != nil {
		return nil, err
	}
	client.servers = servers
	return client, nil
}

func newServer(
	ctx context.Context,
	client *Client,
	outbound routing.Dispatcher,
	ns *NameServer,
	clientIP net.IP,
	container router.GeoIPMatcherContainer,
	matcherInfos *[]DomainMatcherInfo,
	updateDomainRule func(strmatcher.Matcher, int, []DomainMatcherInfo) error) (*Server, error) {
	// Establish domain rules
	var rules []string
	ruleCurr := 0
	ruleIter := 0
	for _, domain := range ns.PrioritizedDomain {
		domainRule, err := ToStrMatcher(domain.Type, domain.Domain)
		if err != nil {
			return nil, newError("failed to create prioritized domain").Base(err).AtWarning()
		}
		originalRuleIdx := ruleCurr
		if ruleCurr < len(ns.OriginalRules) {
			rule := ns.OriginalRules[ruleCurr]
			if ruleCurr >= len(rules) {
				rules = append(rules, rule.Rule)
			}
			ruleIter++
			if ruleIter >= int(rule.Size) {
				ruleIter = 0
				ruleCurr++
			}
		} else { // No original rule, generate one according to current domain matcher (majorly for compatibility with tests)
			rules = append(rules, domainRule.String())
			ruleCurr++
		}
		err = updateDomainRule(domainRule, originalRuleIdx, *matcherInfos)
		if err != nil {
			return nil, newError("failed to create prioritized domain").Base(err).AtWarning()
		}
	}

	// Establish expected IPs
	var matchers []*router.GeoIPMatcher
	for _, geoip := range ns.Geoip {
		matcher, err := container.Add(geoip)
		if err != nil {
			return nil, newError("failed to create ip matcher").Base(err).AtWarning()
		}
		matchers = append(matchers, matcher)
	}

	if len(clientIP) > 0 {
		switch ns.Address.Address.GetAddress().(type) {
		case *net.IPOrDomain_Domain:
			newError("DNS: client ", ns.Address.Address.GetDomain(), " uses clientIP ", clientIP.String()).AtInfo().WriteToLog()
		case *net.IPOrDomain_Ip:
			newError("DNS: client ", ns.Address.Address.GetIp(), " uses clientIP ", clientIP.String()).AtInfo().WriteToLog()
		}
	}

	server := &Server{
		clientIP:     clientIP,
		skipFallback: ns.SkipFallback,
		domains:      rules,
		expectIPs:    matchers,
		concurrency:  ns.Concurrency,
	}

	var name string
	var transport dns.Transport
	destination := ns.Address.AsDestination()
	trans := &transportContext{
		ctx:    ctx,
		client: client,
		server: server,
	}

	if !destination.Address.Family().IsDomain() {
		name = "UDP//" + destination.Address.String()
		destination.Network = net.Network_UDP
		if destination.Port == 0 {
			destination.Port = 53
		} else {
			name += ":" + fmt.Sprint(destination.Port)
		}
		trans.destination = destination
		transport = NewUDPTransport(trans, outbound)
	} else {
		link, err := url.Parse(destination.Address.Domain())
		if err != nil {
			return nil, newError("failed to parse dns server url").Base(err)
		}
		switch link.Scheme {
		case "tcp":
			name = "TCP//" + link.Hostname()
			destination.Network = net.Network_TCP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 53
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.TCPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewTCPTransport(trans, outbound)
		case "tcp+local":
			name = "TCPL//" + link.Hostname()
			destination.Network = net.Network_TCP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 53
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.TCPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewTCPLocalTransport(trans)
		case "udp":
			name = "UDP//" + link.Hostname()
			destination.Network = net.Network_UDP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 53
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.UDPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewUDPTransport(trans, outbound)
		case "udp+local":
			name = "UDPL//" + link.Hostname()
			destination.Network = net.Network_UDP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 53
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.UDPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewUDPLocalTransport(trans)
		case "tls":
			name = "DOT//" + link.Hostname()
			destination.Network = net.Network_TCP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 853
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.TCPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewTLSTransport(trans, outbound)
		case "tls+local":
			name = "DOTL//" + link.Hostname()
			destination.Network = net.Network_TCP
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 853
			} else {
				name += ":" + fmt.Sprint(port)
			}
			trans.destination = net.TCPDestination(net.ParseAddress(link.Hostname()), port)
			transport = NewTLSLocalTransport(trans)
		case "https":
			name = "DOH//" + link.String()
			trans.destination = destination
			transport = NewHTTPSTransport(trans, outbound)
		case "https+local":
			link.Scheme = "https"
			destination.Address = net.DomainAddress(link.String())
			link.Scheme = ""
			name = "DOHL//" + link.String()
			trans.destination = destination
			transport = NewHTTPSLocalTransport(trans)
		case "quic":
			name = "DOQ//" + link.Hostname()
			destination.Network = net.Network_UDP
			destination.Address = net.DomainAddress(link.Hostname())
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 784
			} else {
				name += ":" + fmt.Sprint(port)
			}
			destination.Port = port
			trans.destination = destination
			transport = NewQUICTransport(trans, outbound)
		case "quic+local":
			name = "DOQL//" + link.Hostname()
			destination.Network = net.Network_UDP
			destination.Address = net.DomainAddress(link.Hostname())
			port := net.Port(0)
			portStr := link.Port()
			if portStr != "" {
				port, err = net.PortFromString(link.Port())
				if err != nil {
					return nil, err
				}
			}
			if port == 0 {
				port = 784
			} else {
				name += ":" + fmt.Sprint(port)
			}
			destination.Port = port
			trans.destination = destination
			transport = NewQUICLocalTransport(trans)
		default:
			switch link.String() {
			case "localhost":
				name = "localhost"
				transport = localdns.Transport()
			default:
				return nil, newError("failed to create dns server: ", link.String())
			}
		}

	}

	server.name = name
	server.transport = transport

	if _, isLocalNameServer := transport.(localdns.LocalTransport); isLocalNameServer {
		ns.PrioritizedDomain = append(ns.PrioritizedDomain, LocalTLDsAndDotlessDomains...)
		ns.OriginalRules = append(ns.OriginalRules, LocalTLDsAndDotlessDomainsRule)
		// The following lines is a solution to avoid core panics（rule index out of range） when setting `localhost` DNS client in config.
		// Because the `localhost` DNS client will apend len(localTLDsAndDotlessDomains) rules into matcherInfos to match `geosite:private` default rule.
		// But `matcherInfos` has no enough length to add rules, which leads to core panics (rule index out of range).
		// To avoid this, the length of `matcherInfos` must be equal to the expected, so manually append it with Golang default zero value first for later modification.
		// Related issues:
		// https://github.com/v2fly/v2ray-core/issues/529
		// https://github.com/v2fly/v2ray-core/issues/719
		for i := 0; i < len(LocalTLDsAndDotlessDomains); i++ {
			*matcherInfos = append(*matcherInfos, DomainMatcherInfo{
				ClientIdx:     uint16(0),
				DomainRuleIdx: uint16(0),
			})
		}
	}

	return server, nil
}

func (c *Client) sortServers(domain string) []*Server {
	clients := make([]*Server, 0, len(c.servers))
	clientUsed := make([]bool, len(c.servers))
	clientNames := make([]string, 0, len(c.servers))
	var domainRules []string

	// Priority domain matching
	hasMatch := false
	for _, match := range c.domainMatcher.Match(domain) {
		info := c.matcherInfos[match]
		client := c.servers[info.ClientIdx]
		domainRule := client.domains
		domainRules = append(domainRules, fmt.Sprintf("%s(DNS idx:%d)", domainRule, info.ClientIdx))
		if clientUsed[info.ClientIdx] {
			continue
		}
		clientUsed[info.ClientIdx] = true
		clients = append(clients, client)
		clientNames = append(clientNames, client.name)
		hasMatch = true
	}

	if !(c.disableFallback || c.disableFallbackIfMatch && hasMatch) {
		// Default round-robin query
		for idx, client := range c.servers {
			if clientUsed[idx] || client.skipFallback {
				continue
			}
			clientUsed[idx] = true
			clients = append(clients, client)
			clientNames = append(clientNames, client.name)
		}
	}

	if len(domainRules) > 0 {
		newError("domain ", domain, " matches following rules: ", domainRules).AtDebug().WriteToLog()
	}
	if len(clientNames) > 0 {
		newError("domain ", domain, " will use DNS in order: ", clientNames).AtDebug().WriteToLog()
	}

	if len(clients) == 0 {
		clients = append(clients, c.servers[0])
		clientNames = append(clientNames, c.servers[0].name)
		newError("domain ", domain, " will use the first DNS: ", clientNames).AtDebug().WriteToLog()
	}

	return clients
}

func (c *Server) matchExpectedIPs(domain string, ips []net.IP) ([]net.IP, error) {
	if len(c.expectIPs) == 0 {
		return ips, nil
	}
	var newIps []net.IP
	for _, ip := range ips {
		for _, matcher := range c.expectIPs {
			if matcher.Match(ip) {
				newIps = append(newIps, ip)
				break
			}
		}
	}
	if len(newIps) == 0 {
		return nil, ErrExpectedIPNonMatch
	}
	newError("domain ", domain, " expectIPs ", newIps, " matched at server ", c.name).AtDebug().WriteToLog()
	return newIps, nil
}

var typeMap = map[DomainMatchingType]strmatcher.Type{
	DomainMatchingType_Full:      strmatcher.Full,
	DomainMatchingType_Subdomain: strmatcher.Domain,
	DomainMatchingType_Keyword:   strmatcher.Substr,
	DomainMatchingType_Regex:     strmatcher.Regex,
}

// References:
// https://www.iana.org/assignments/special-use-domain-names/special-use-domain-names.xhtml
// https://unix.stackexchange.com/questions/92441/whats-the-difference-between-local-home-and-lan
var LocalTLDsAndDotlessDomains = []*NameServer_PriorityDomain{
	{Type: DomainMatchingType_Regex, Domain: "^[^.]+$"}, // This will only match domains without any dot
	{Type: DomainMatchingType_Subdomain, Domain: "local"},
	{Type: DomainMatchingType_Subdomain, Domain: "localdomain"},
	{Type: DomainMatchingType_Subdomain, Domain: "localhost"},
	{Type: DomainMatchingType_Subdomain, Domain: "lan"},
	{Type: DomainMatchingType_Subdomain, Domain: "home.arpa"},
	{Type: DomainMatchingType_Subdomain, Domain: "example"},
	{Type: DomainMatchingType_Subdomain, Domain: "invalid"},
	{Type: DomainMatchingType_Subdomain, Domain: "test"},
}

var LocalTLDsAndDotlessDomainsRule = &NameServer_OriginalRule{
	Rule: "geosite:private",
	Size: uint32(len(LocalTLDsAndDotlessDomains)),
}

func ToStrMatcher(t DomainMatchingType, domain string) (strmatcher.Matcher, error) {
	strMType, f := typeMap[t]
	if !f {
		return nil, newError("unknown mapping type", t).AtWarning()
	}
	matcher, err := strMType.New(domain)
	if err != nil {
		return nil, newError("failed to create str matcher").Base(err)
	}
	return matcher, nil
}

func toNetIP(addrs []net.Address) ([]net.IP, error) {
	ips := make([]net.IP, 0, len(addrs))
	for _, addr := range addrs {
		if addr.Family().IsIP() {
			ips = append(ips, addr.IP())
		} else {
			return nil, newError("Failed to convert address", addr, "to Net IP.").AtWarning()
		}
	}
	return ips, nil
}

func GenerateRandomTag() string {
	id := uuid.New()
	return "v2ray.system." + id.String()
}

// Fqdn normalizes domain make sure it ends with '.'
func Fqdn(domain string) string {
	if len(domain) > 0 && strings.HasSuffix(domain, ".") {
		return domain
	}
	return domain + "."
}

func genEDNS0Options(clientIP net.IP) *dnsmessage.Resource {
	if len(clientIP) == 0 {
		return nil
	}

	var netmask int
	var family uint16

	if len(clientIP) == 4 {
		family = 1
		netmask = 24 // 24 for IPV4, 96 for IPv6
	} else {
		family = 2
		netmask = 96
	}

	b := make([]byte, 4)
	binary.BigEndian.PutUint16(b[0:], family)
	b[2] = byte(netmask)
	b[3] = 0
	switch family {
	case 1:
		ip := clientIP.To4().Mask(net.CIDRMask(netmask, net.IPv4len*8))
		needLength := (netmask + 8 - 1) / 8 // division rounding up
		b = append(b, ip[:needLength]...)
	case 2:
		ip := clientIP.Mask(net.CIDRMask(netmask, net.IPv6len*8))
		needLength := (netmask + 8 - 1) / 8 // division rounding up
		b = append(b, ip[:needLength]...)
	}

	const EDNS0SUBNET = 0x08

	opt := new(dnsmessage.Resource)
	common.Must(opt.Header.SetEDNS0(1350, 0xfe00, true))

	opt.Body = &dnsmessage.OPTResource{
		Options: []dnsmessage.Option{
			{
				Code: EDNS0SUBNET,
				Data: b,
			},
		},
	}

	return opt
}

// DomainMatcherInfo contains information attached to index returned by Server.domainMatcher
type DomainMatcherInfo struct {
	ClientIdx     uint16
	DomainRuleIdx uint16
}
