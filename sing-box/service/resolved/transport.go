package resolved

import (
	"context"
	"net/netip"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/dialer"
	"github.com/sagernet/sing-box/common/tls"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/dns"
	"github.com/sagernet/sing-box/dns/transport"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	M "github.com/sagernet/sing/common/metadata"
	"github.com/sagernet/sing/service"

	mDNS "github.com/miekg/dns"
)

func RegisterTransport(registry *dns.TransportRegistry) {
	dns.RegisterTransport[option.SplitDNSServerOptions](registry, C.TypeResolved, NewTransport)
}

var _ adapter.DNSTransport = (*Transport)(nil)

type Transport struct {
	dns.TransportAdapter
	ctx                    context.Context
	logger                 logger.ContextLogger
	serviceTag             string
	acceptDefaultResolvers bool
	ndots                  int
	timeout                time.Duration
	attempts               int
	rotate                 bool
	service                *Service
	linkAccess             sync.RWMutex
	linkServers            map[*TransportLink]*LinkServers
}

type LinkServers struct {
	Link         *TransportLink
	Servers      []adapter.DNSTransport
	serverOffset uint32
}

func (c *LinkServers) ServerOffset(rotate bool) uint32 {
	if rotate {
		return atomic.AddUint32(&c.serverOffset, 1) - 1
	}
	return 0
}

func NewTransport(ctx context.Context, logger log.ContextLogger, tag string, options option.SplitDNSServerOptions) (adapter.DNSTransport, error) {
	if !C.IsLinux {
		return nil, E.New("split DNS server is only supported on Linux")
	}
	return &Transport{
		TransportAdapter:       dns.NewTransportAdapter(C.DNSTypeDHCP, tag, nil),
		ctx:                    ctx,
		logger:                 logger,
		serviceTag:             options.Service,
		acceptDefaultResolvers: options.AcceptDefaultResolvers,
		// ndots:                  options.NDots,
		// timeout:                time.Duration(options.Timeout),
		// attempts:               options.Attempts,
		// rotate:                 options.Rotate,
		ndots:       1,
		timeout:     5 * time.Second,
		attempts:    2,
		linkServers: make(map[*TransportLink]*LinkServers),
	}, nil
}

func (t *Transport) Start(stage adapter.StartStage) error {
	if stage != adapter.StartStateInitialize {
		return nil
	}
	serviceManager := service.FromContext[adapter.ServiceManager](t.ctx)
	service, loaded := serviceManager.Get(t.serviceTag)
	if !loaded {
		return E.New("service not found: ", t.serviceTag)
	}
	resolvedInbound, isResolved := service.(*Service)
	if !isResolved {
		return E.New("service is not resolved: ", t.serviceTag)
	}
	resolvedInbound.updateCallback = t.updateTransports
	t.service = resolvedInbound
	return nil
}

func (t *Transport) Close() error {
	t.linkAccess.RLock()
	defer t.linkAccess.RUnlock()
	for _, servers := range t.linkServers {
		for _, server := range servers.Servers {
			server.Close()
		}
	}
	return nil
}

func (t *Transport) updateTransports(link *TransportLink) error {
	t.linkAccess.Lock()
	defer t.linkAccess.Unlock()
	if servers, loaded := t.linkServers[link]; loaded {
		for _, server := range servers.Servers {
			server.Close()
		}
	}
	serverDialer := common.Must1(dialer.NewDefault(t.ctx, option.DialerOptions{
		BindInterface:      link.iif.Name,
		UDPFragmentDefault: true,
	}))
	var transports []adapter.DNSTransport
	for _, address := range link.address {
		serverAddr, ok := netip.AddrFromSlice(address.Address)
		if !ok {
			return os.ErrInvalid
		}
		if link.dnsOverTLS {
			tlsConfig := common.Must1(tls.NewClient(t.ctx, serverAddr.String(), option.OutboundTLSOptions{
				Enabled:    true,
				ServerName: serverAddr.String(),
			}))
			transports = append(transports, transport.NewTLSRaw(t.logger, t.TransportAdapter, serverDialer, M.SocksaddrFrom(serverAddr, 53), tlsConfig))

		} else {
			transports = append(transports, transport.NewUDPRaw(t.logger, t.TransportAdapter, serverDialer, M.SocksaddrFrom(serverAddr, 53)))
		}
	}
	for _, address := range link.addressEx {
		serverAddr, ok := netip.AddrFromSlice(address.Address)
		if !ok {
			return os.ErrInvalid
		}
		if link.dnsOverTLS {
			var serverName string
			if address.Name != "" {
				serverName = address.Name
			} else {
				serverName = serverAddr.String()
			}
			tlsConfig := common.Must1(tls.NewClient(t.ctx, serverAddr.String(), option.OutboundTLSOptions{
				Enabled:    true,
				ServerName: serverName,
			}))
			transports = append(transports, transport.NewTLSRaw(t.logger, t.TransportAdapter, serverDialer, M.SocksaddrFrom(serverAddr, address.Port), tlsConfig))

		} else {
			transports = append(transports, transport.NewUDPRaw(t.logger, t.TransportAdapter, serverDialer, M.SocksaddrFrom(serverAddr, address.Port)))
		}
	}
	t.linkServers[link] = &LinkServers{
		Link:    link,
		Servers: transports,
	}
	return nil
}

func (t *Transport) Exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	question := message.Question[0]
	var selectedLink *TransportLink
	for _, link := range t.service.links {
		for _, domain := range link.domain {
			if strings.HasSuffix(question.Name, domain.Domain) {
				selectedLink = link
			}
		}
	}
	if selectedLink == nil && t.acceptDefaultResolvers {
		for _, link := range t.service.links {
			if link.defaultRoute {
				selectedLink = link
			}
		}
	}
	if selectedLink == nil {
		t.logger.DebugContext(ctx, "missing selected interface")
		return dns.FixedResponseStatus(message, mDNS.RcodeNameError), nil
	}
	servers := t.linkServers[selectedLink]
	if len(servers.Servers) == 0 {
		t.logger.DebugContext(ctx, "missing DNS servers")
		return dns.FixedResponseStatus(message, mDNS.RcodeNameError), nil
	}
	if question.Qtype == mDNS.TypeA || question.Qtype == mDNS.TypeAAAA {
		return t.exchangeParallel(ctx, servers, message)
	} else {
		return t.exchangeSingleRequest(ctx, servers, message)
	}
}

func (t *Transport) exchangeSingleRequest(ctx context.Context, servers *LinkServers, message *mDNS.Msg) (*mDNS.Msg, error) {
	var lastErr error
	for _, fqdn := range servers.Link.nameList(t.ndots, message.Question[0].Name) {
		response, err := t.tryOneName(ctx, servers, message, fqdn)
		if err != nil {
			lastErr = err
			continue
		}
		return response, nil
	}
	return nil, lastErr
}

func (t *Transport) tryOneName(ctx context.Context, servers *LinkServers, message *mDNS.Msg, fqdn string) (*mDNS.Msg, error) {
	serverOffset := servers.ServerOffset(t.rotate)
	sLen := uint32(len(servers.Servers))
	var lastErr error
	for i := 0; i < t.attempts; i++ {
		for j := uint32(0); j < sLen; j++ {
			server := servers.Servers[(serverOffset+j)%sLen]
			question := message.Question[0]
			question.Name = fqdn
			exchangeMessage := *message
			exchangeMessage.Question = []mDNS.Question{question}
			exchangeCtx, cancel := context.WithTimeout(ctx, t.timeout)
			response, err := server.Exchange(exchangeCtx, &exchangeMessage)
			cancel()
			if err != nil {
				lastErr = err
				continue
			}
			return response, nil
		}
	}
	return nil, E.Cause(lastErr, fqdn)
}

func (t *Transport) exchangeParallel(ctx context.Context, servers *LinkServers, message *mDNS.Msg) (*mDNS.Msg, error) {
	returned := make(chan struct{})
	defer close(returned)
	type queryResult struct {
		response *mDNS.Msg
		err      error
	}
	results := make(chan queryResult)
	startRacer := func(ctx context.Context, fqdn string) {
		response, err := t.tryOneName(ctx, servers, message, fqdn)
		select {
		case results <- queryResult{response, err}:
		case <-returned:
		}
	}
	queryCtx, queryCancel := context.WithCancel(ctx)
	defer queryCancel()
	var nameCount int
	for _, fqdn := range servers.Link.nameList(t.ndots, message.Question[0].Name) {
		nameCount++
		go startRacer(queryCtx, fqdn)
	}
	var errors []error
	for {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case result := <-results:
			if result.err == nil {
				return result.response, nil
			}
			errors = append(errors, result.err)
			if len(errors) == nameCount {
				return nil, E.Errors(errors...)
			}
		}
	}
}
