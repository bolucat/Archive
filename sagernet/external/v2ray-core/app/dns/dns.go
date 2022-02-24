package dns

import (
	"context"
	"encoding/binary"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"golang.org/x/net/dns/dnsmessage"
	"net/netip"

	"github.com/v2fly/v2ray-core/v5/app/router"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/errors"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/common/strmatcher"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
)

//go:generate go run github.com/v2fly/v2ray-core/v5/common/errors/errorgen

var _ dns.NewClient = (*Client)(nil)

type Client struct {
	access sync.Mutex
	ctx    context.Context
	cancel context.CancelFunc

	tag                  string
	clientIP             net.IP
	domainMatcher        strmatcher.IndexMatcher
	matcherInfos         []DomainMatcherInfo
	defaultQueryStrategy dns.QueryStrategy
	hosts                *StaticHosts
	servers              []*Server

	disableCache           bool
	disableFallback        bool
	disableFallbackIfMatch bool
	disableExpire          bool

	requestId int32
	callbacks sync.Map
	cache     sync.Map
}

type Server struct {
	name         string
	transport    dns.Transport
	clientIP     net.IP
	skipFallback bool
	domains      []string
	expectIPs    []*router.GeoIPMatcher
	concurrency  bool
	access       sync.Mutex
}

type transportContext struct {
	ctx         context.Context
	client      *Client
	server      *Server
	destination net.Destination

	cache   *buf.Buffer
	missing int32
}

type queryCallback struct {
	parseIPs bool
	domain   string
	strategy dns.QueryStrategy

	wg *sync.WaitGroup

	ctx    context.Context
	cancel context.CancelFunc

	access   sync.Mutex
	response *serverQueryCallback
}

type serverQueryCallback struct {
	*queryCallback

	ctx              context.Context
	cancel           context.CancelFunc
	finish4, finish6 bool

	ttl       uint32
	queryType sync.Map
	access    sync.Mutex
	message   *dnsmessage.Message
	ips       []net.IP
	errors    []error
}

type ipCacheEntire struct {
	ttl              uint32
	cached4, cached6 bool
	cache4, cache6   []net.IP
	expire4, expire6 time.Time
}

func (c *Client) nextRequestId() uint16 {
	requestId := atomic.AddInt32(&c.requestId, 1)
	if requestId > 65535 {
		requestId = 1
		atomic.AddInt32(&c.requestId, -65534)
	}
	return uint16(requestId)
}

func (c *Client) LookupDefault(ctx context.Context, domain string) ([]net.IP, uint32, error) {
	return c.Lookup(ctx, domain, c.defaultQueryStrategy)
}

func (c *Client) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, uint32, error) {
	var ttl uint32
	if strings.HasSuffix(domain, ".") {
		domain = domain[:len(domain)-1]
	}

	var ips []net.IP
	var cached4, cached6 bool
	now := time.Now()

	cacheI, cachedHit := c.cache.Load(domain)
	if cachedHit {
		cache := cacheI.(*ipCacheEntire)
		ttl = cache.ttl
		if strategy != dns.QueryStrategy_USE_IP6 {
			if cache.cached4 && (c.disableExpire || now.Before(cache.expire4)) {
				ips = append(ips, cache.cache4...)
				cached4 = true
			}
		}
		if strategy != dns.QueryStrategy_USE_IP4 {
			if cache.cached6 && (c.disableExpire || now.Before(cache.expire6)) {
				ips = append(ips, cache.cache6...)
				cached6 = true
			}
		}
	}

	if len(ips) > 0 {
		newError("dns cache HIT ", domain, " -> ", ips).AtDebug().WriteToLog()
	}

	var query bool
	switch strategy {
	case dns.QueryStrategy_USE_IP4:
		query = !cached4
	case dns.QueryStrategy_USE_IP6:
		query = !cached6
	default:
		query = !cached4 || !cached6
	}

	newStrategy := strategy
	if query {
		if cached4 {
			newStrategy = dns.QueryStrategy_USE_IP6
		}
		if cached6 {
			newStrategy = dns.QueryStrategy_USE_IP4
		}
	} else if len(ips) == 0 {
		return nil, ttl, dns.ErrEmptyResponse
	}

	if query {
		queried, ttl, err := c.lookup(ctx, domain, newStrategy)
		if err != nil {
			return nil, ttl, err
		}
		ips = append(ips, queried...)
	}

	if strategy == dns.QueryStrategy_PREFER_IP4 {
		var newIPs []net.IP
		for _, ip := range ips {
			if len(ip) == net.IPv4len {
				newIPs = append(newIPs, ip)
			}
		}
		for _, ip := range ips {
			if len(ip) == net.IPv6len {
				newIPs = append(newIPs, ip)
			}
		}
	} else if strategy == dns.QueryStrategy_PREFER_IP6 {
		var newIPs []net.IP
		for _, ip := range ips {
			if len(ip) == net.IPv6len {
				newIPs = append(newIPs, ip)
			}
		}
		for _, ip := range ips {
			if len(ip) == net.IPv4len {
				newIPs = append(newIPs, ip)
			}
		}
	}

	return ips, ttl, nil
}

func (c *Client) lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, uint32, error) {
	servers := c.sortServers(domain)
	var messages []*dnsmessage.Message

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	name, err := dnsmessage.NewName(Fqdn(domain))
	if err != nil {
		return nil, 0, newError("failed to create domain query").Base(err)
	}

	{
		message := new(dnsmessage.Message)
		message.Header.ID = c.nextRequestId()
		message.Header.RecursionDesired = true
		if strategy != dns.QueryStrategy_USE_IP6 {
			message4 := *message
			messages = append(messages, &message4)
			message4.Questions = append(message.Questions, dnsmessage.Question{
				Name:  name,
				Type:  dnsmessage.TypeA,
				Class: dnsmessage.ClassINET,
			})
		}
		if strategy != dns.QueryStrategy_USE_IP4 {
			message6 := *message
			messages = append(messages, &message6)
			message6.Questions = append(message.Questions, dnsmessage.Question{
				Name:  name,
				Type:  dnsmessage.TypeAAAA,
				Class: dnsmessage.ClassINET,
			})
		}
	}

	q := &queryCallback{
		wg:     new(sync.WaitGroup),
		ctx:    ctx,
		cancel: cancel,

		parseIPs: true,
		domain:   domain,
		strategy: strategy,
	}

	q.wg.Add(len(servers))

	var reqIds []uint16
	var requests []*serverQueryCallback

	for _, server := range servers {
		server := server
		ctx, cancel := context.WithCancel(ctx)
		r := &serverQueryCallback{
			queryCallback: q,
			ctx:           ctx,
			cancel:        cancel,
		}
		go func() {
			<-ctx.Done()
			r.wg.Done()
		}()
		requests = append(requests, r)
		switch server.transport.Type() {
		case dns.TransportTypeDefault:
			for index := range messages {
				message := messages[index]
				message.ID = c.nextRequestId()
				reqIds = append(reqIds, message.ID)
				r.queryType.Store(message.ID, message.Questions[0].Type)
				c.callbacks.Store(message.ID, r)
				go func() {
					if err := server.transport.Write(ctx, message); err != nil {
						r.errors = append(r.errors, newError("failed write query to dns server ", server.name).Base(err))
						cancel()
					}
				}()
			}
		case dns.TransportTypeExchange:
			for index := range messages {
				message := messages[index]
				message.ID = c.nextRequestId()
				reqIds = append(reqIds, message.ID)
				r.queryType.Store(message.ID, message.Questions[0].Type)
				c.callbacks.Store(message.ID, r)
				go func() {
					response, err := server.transport.Exchange(ctx, message)
					if err != nil {
						r.errors = append(r.errors, err)
						cancel()
						return
					}
					c.writeBack(server, response)
				}()
			}
		case dns.TransportTypeExchangeRaw:
			for index := range messages {
				message := messages[index]
				message.ID = c.nextRequestId()
				packed, err := message.Pack()
				if err != nil {
					r.errors = append(r.errors, newError("failed to pack dns query").Base(err))
					cancel()
					continue
				}
				reqIds = append(reqIds, message.ID)
				r.queryType.Store(message.ID, message.Questions[0].Type)
				c.callbacks.Store(message.ID, r)
				go func() {
					response, err := server.transport.ExchangeRaw(ctx, buf.FromBytes(packed))
					if err != nil {
						r.errors = append(r.errors, err)
						cancel()
						return
					}
					c.writeBackRaw(server, response)
				}()
			}
		case dns.TransportTypeLookup:
			go func() {
				ips, err := server.transport.Lookup(ctx, domain, strategy)
				q.access.Lock()
				defer q.access.Unlock()
				if err != nil {
					r.errors = append(r.errors, err)
				} else if !common.Done(ctx) {
					matched, err := server.matchExpectedIPs(r.domain, ips)
					if err != nil {
						r.errors = append(r.errors, err)
						return
					}
					newError(server.name, " got answer: ", r.domain, " -> ", ips).AtDebug().WriteToLog(session.ExportIDToError(ctx))
					r.ips = matched
					q.response = r
					q.cancel()
				}
				cancel()
			}()
		}
		if !server.concurrency {
			r.wg.Wait()
			if common.Done(r.ctx) {
				break
			}
		}
	}

	task.Run(ctx, func() error {
		q.wg.Wait()
		return nil
	})

	cancel()

	for _, reqId := range reqIds {
		c.callbacks.Delete(reqId)
	}

	response := q.response
	if response != nil {
		ips := q.response.ips
		ttl := response.ttl
		if len(ips) == 0 {
			return nil, ttl, dns.ErrEmptyResponse
		}
		return ips, ttl, nil
	}

	for _, request := range requests {
		for _, err := range request.errors {
			if _, code := err.(dns.RCodeError); code {
				return nil, 0, err
			}
		}
	}

	var errs []error
	for _, request := range requests {
		errs = append(errs, request.errors...)
	}
	err = errors.Combine(errs...)

	if err == nil {
		err = context.Canceled
	}
	return nil, 0, err
}

func (c *Client) QueryRaw(ctx context.Context, buffer *buf.Buffer) (*buf.Buffer, error) {
	message := &dnsmessage.Message{}
	if err := message.Unpack(buffer.Bytes()); err != nil {
		buffer.Release()
		return nil, newError("failed to parse dns request").Base(err)
	}
	messageID := message.ID
	if common.IsEmpty(message.Questions) {
		return packMessage(&dnsmessage.Message{
			Header: dnsmessage.Header{
				ID:            messageID,
				Response:      true,
				RCode:         dnsmessage.RCodeFormatError,
				Authoritative: true,
			},
		})
	}
	message.Questions = message.Questions[:1]
	domain := message.Questions[0].Name.String()
	if strings.HasSuffix(domain, ".") {
		domain = domain[:len(domain)-1]
	}

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	servers := c.sortServers(domain)

	q := &queryCallback{
		wg:     new(sync.WaitGroup),
		ctx:    ctx,
		cancel: cancel,
		domain: domain,
	}
	q.wg.Add(len(servers))
	var reqIds []uint16
	var requests []*serverQueryCallback
	for _, server := range servers {
		server := server
		ctx, cancel := context.WithCancel(ctx)
		r := &serverQueryCallback{
			queryCallback: q,
			ctx:           ctx,
			cancel:        cancel,
		}
		go func() {
			<-ctx.Done()
			r.wg.Done()
		}()
		switch server.transport.Type() {
		case dns.TransportTypeDefault:
			message.ID = c.nextRequestId()
			c.callbacks.Store(message.ID, r)
			reqIds = append(reqIds, message.ID)
			go func() {
				if err := server.transport.Write(ctx, message); err != nil {
					r.errors = append(r.errors, newError("failed write query to dns server ", server.name).Base(err))
					cancel()
				}
			}()
		case dns.TransportTypeExchange:
			message.ID = c.nextRequestId()
			r.queryType.Store(message.ID, message.Questions[0].Type)
			c.callbacks.Store(message.ID, r)
			reqIds = append(reqIds, message.ID)
			go func() {
				response, err := server.transport.Exchange(ctx, message)
				if err != nil {
					r.errors = append(r.errors, err)
					cancel()
					return
				}
				c.writeBack(server, response)
			}()
		case dns.TransportTypeExchangeRaw:
			message.ID = c.nextRequestId()
			packed, err := message.Pack()
			if err != nil {
				r.errors = append(r.errors, newError("failed to pack dns query").Base(err))
				cancel()
				continue
			}
			c.callbacks.Store(message.ID, r)
			reqIds = append(reqIds, message.ID)
			go func() {
				response, err := server.transport.ExchangeRaw(ctx, buf.FromBytes(packed))
				if err != nil {
					r.errors = append(r.errors, err)
					cancel()
					return
				}
				c.writeBackRaw(server, response)
			}()
		case dns.TransportTypeLookup:
			var strategy dns.QueryStrategy
			if message.Questions[0].Class == dnsmessage.ClassINET {
				switch message.Questions[0].Type {
				case dnsmessage.TypeA:
					strategy = dns.QueryStrategy_USE_IP4
				case dnsmessage.TypeAAAA:
					strategy = dns.QueryStrategy_USE_IP6
				}
			}
			if strategy == dns.QueryStrategy_USE_IP {
				r.errors = append(r.errors, common.ErrNoClue)
				cancel()
				continue
			}
			go func() {
				ips, err := server.transport.Lookup(ctx, domain, strategy)
				q.access.Lock()
				defer q.access.Unlock()
				if err != nil {
					r.errors = append(r.errors, err)
				} else if !common.Done(ctx) {
					matched, err := server.matchExpectedIPs(r.domain, ips)
					if err != nil {
						r.errors = append(r.errors, err)
						return
					}
					newError(server.name, " got answer: ", r.domain, " -> ", ips).AtDebug().WriteToLog(session.ExportIDToError(ctx))
					r.ips = matched
					q.response = r
					q.cancel()
				}
				cancel()
			}()
		}

		if !server.concurrency {
			r.wg.Wait()
			if common.Done(r.ctx) {
				break
			}
		}
	}

	task.Run(ctx, func() error {
		q.wg.Wait()
		return nil
	})

	cancel()

	for _, reqId := range reqIds {
		c.callbacks.Delete(reqId)
	}

	response := q.response
	if response != nil && response.message != nil {
		responseMessage := response.message
		responseMessage.ID = messageID
		return packMessage(responseMessage)
	}

	for _, request := range requests {
		if request.message != nil {
			responseMessage := response.message
			responseMessage.ID = messageID
			return packMessage(responseMessage)
		}
	}

	for _, request := range requests {
		for _, err := range request.errors {
			if rErr, is := err.(dns.RCodeError); is {
				return packMessage(&dnsmessage.Message{
					Header: dnsmessage.Header{
						ID:            messageID,
						Response:      true,
						RCode:         dnsmessage.RCode(rErr),
						Authoritative: true,
					},
				})
			}
		}
	}

	var errs []error
	for _, request := range requests {
		errs = append(errs, request.errors...)
	}
	err := errors.Combine(errs...)
	if err == nil {
		err = context.Canceled
	}

	newError("failed to process raw dns query for domain ", domain).Base(err).AtWarning().WriteToLog(session.ExportIDToError(ctx))

	return packMessage(&dnsmessage.Message{
		Header: dnsmessage.Header{
			ID:            messageID,
			Response:      true,
			RCode:         dnsmessage.RCodeServerFailure,
			Authoritative: true,
		},
	})
}

func packMessage(message *dnsmessage.Message) (*buf.Buffer, error) {
	packed, err := message.Pack()
	if err != nil {
		return nil, err
	}
	return buf.FromBytes(packed), nil
}

func (c *transportContext) newContext() context.Context {
	return session.ContextWithContent(c.ctx, &session.Content{Protocol: "v2ray.dns"})
}

func (c *transportContext) writeBack(message *dnsmessage.Message) {
	c.client.writeBack(c.server, message)
}

func (c *transportContext) writeBackRaw(buffer *buf.Buffer) {
	c.client.writeBackRaw(c.server, buffer)
}

func (c *transportContext) writeBackRawTCP(message *buf.Buffer) {
	if c.missing == 0 {
		if message.Len() < 2 {
			c.cache = message
			c.missing = message.Len() - 2
			return
		}

		messageLen := int32(binary.BigEndian.Uint16(message.BytesTo(2)))
		message.Advance(2)
		missing := messageLen - message.Len()
		if missing == 0 {
			c.writeBackRaw(message)
			return
		}
		if missing < 0 {
			c.writeBackRaw(buf.FromBytes(message.BytesTo(messageLen)))
			message.Advance(messageLen)
			c.writeBackRawTCP(message)
			return
		}
		c.cache = message
		c.missing = missing
	} else if c.missing < 0 {
		missing := -c.missing
		if message.Len() < missing {
			c.cache.Write(message.Bytes())
			c.missing -= message.Len()
			message.Release()
			return
		}
		message.Advance(missing)
		messageLen := int32(binary.BigEndian.Uint16(c.cache.BytesTo(2)))
		c.cache.Release()

		missing = messageLen - message.Len()
		if missing == 0 {
			c.cache = nil
			c.missing = 0
			c.writeBackRaw(message)
			return
		}
		if missing < 0 {
			c.cache = nil
			c.missing = 0
			c.writeBackRaw(buf.FromBytes(message.BytesTo(messageLen)))
			message.Advance(messageLen)
			c.writeBackRawTCP(message)
			return
		}

		c.cache = message
		c.missing = missing
	} else {
		if c.missing > message.Len() {
			c.missing -= message.Len()
			c.cache.Write(message.Bytes())
			message.Release()
			return
		}
		if c.missing == message.Len() {
			c.cache.Write(message.Bytes())
			c.writeBackRaw(c.cache)
			c.missing = 0
			c.cache = nil
			message.Release()
			return
		}

		c.cache.Write(message.BytesTo(c.missing))
		message.Advance(c.missing)
		c.writeBackRaw(c.cache)
		c.missing = 0
		c.cache = nil
		c.writeBackRawTCP(message)
	}
}

func (c *Client) writeBackRaw(server *Server, buffer *buf.Buffer) {
	defer buffer.Release()
	message := new(dnsmessage.Message)
	if err := message.Unpack(buffer.Bytes()); err != nil {
		newError("failed to parse DNS response at server ", server.name).Base(err).WriteToLog()
		return
	}
	c.writeBack(server, message)
}

func (c *Client) writeBack(server *Server, message *dnsmessage.Message) {
	callbackI, loaded := c.callbacks.LoadAndDelete(message.ID)
	if !loaded {
		newError("no callback for response ", message.ID).AtDebug().WriteToLog()
		return
	}

	d := callbackI.(*serverQueryCallback)

	if common.Done(d.ctx) {
		return
	}

	d.access.Lock()
	defer d.access.Unlock()

	if message.RCode != dnsmessage.RCodeSuccess {
		err := dns.RCodeError(message.RCode)
		d.errors = append(d.errors, err)
		d.cancel()
		newError("failed to lookup ip for domain ", d.domain, " at server ", server.name).Base(err).AtDebug().WriteToLog(session.ExportIDToError(d.ctx))
		return
	}

	if !d.parseIPs {
		d.queryCallback.access.Lock()
		defer d.queryCallback.access.Unlock()

		if common.Done(d.ctx) {
			return
		}

		newError(server.name, " got answer for raw query ", message.ID).AtDebug().WriteToLog(session.ExportIDToError(d.ctx))

		d.message = message
		d.queryCallback.response = d
		d.queryCallback.cancel()
		d.cancel()
		return
	}

	typeI, _ := d.queryType.Load(message.ID)
	queryType := typeI.(dnsmessage.Type)

	now := time.Now()

	var has4, has6 bool
	var addr4, addr6 []netip.Addr
	var ttl4, ttl6 uint32

	for _, answer := range message.Answers {
		switch resource := answer.Body.(type) {
		case *dnsmessage.AResource:
			addr4 = append(addr4, netip.AddrFrom4(resource.A))
			if answer.Header.TTL > 0 && (ttl4 == 0 || ttl4 > answer.Header.TTL) {
				ttl4 = answer.Header.TTL
			}
			has4 = true
		case *dnsmessage.AAAAResource:
			addr6 = append(addr6, netip.AddrFrom16(resource.AAAA))
			if answer.Header.TTL > 0 && (ttl6 == 0 || ttl6 > answer.Header.TTL) {
				ttl6 = answer.Header.TTL
			}
			has6 = true
		}
	}

	cache := new(ipCacheEntire)
	if has4 && d.strategy != dns.QueryStrategy_USE_IP6 || queryType == dnsmessage.TypeA {
		cache.cache4 = common.Map(addr4, func(it netip.Addr) net.IP {
			return it.AsSlice()
		})
		cache.cached4 = true
		if ttl4 == 0 {
			ttl4 = 6 * 60
		}
		cache.ttl = ttl4
		cache.expire4 = now.Add(time.Duration(ttl4) * time.Second)
		d.finish4 = true
	}
	if has6 && d.strategy != dns.QueryStrategy_USE_IP4 || queryType == dnsmessage.TypeAAAA {
		cache.cache6 = common.Map(addr6, func(it netip.Addr) net.IP {
			return it.AsSlice()
		})
		cache.cached6 = true
		if ttl6 == 0 {
			ttl6 = 6 * 60
		}
		if cache.ttl == 0 || ttl6 < cache.ttl {
			cache.ttl = ttl6
		}
		cache.expire6 = now.Add(time.Duration(ttl6) * time.Second)
		d.finish6 = true
	}
	cacheI, cacheExists := c.cache.LoadOrStore(d.domain, cache)
	if cacheExists {
		acCache := cacheI.(*ipCacheEntire)
		if cache.cached4 {
			acCache.cache4, acCache.cached4, acCache.expire4 = cache.cache4, cache.cached4, cache.expire4
		}
		if cache.cached6 {
			acCache.cache6, acCache.cached6, acCache.expire6 = cache.cache6, cache.cached6, cache.expire6
		}
		if acCache.ttl == 0 || cache.ttl < acCache.ttl {
			acCache.ttl = cache.ttl
		}
	}
	var ips []net.IP
	if len(addr4) > 0 {
		ips = append(ips, cache.cache4...)
	}
	if len(addr6) > 0 {
		ips = append(ips, cache.cache6...)
	}
	matched, err := server.matchExpectedIPs(d.domain, ips)
	if err != nil {
		return
	}
	d.ips = append(d.ips, matched...)
	d.ttl = cache.ttl

	var finish bool
	switch d.strategy {
	case dns.QueryStrategy_USE_IP4:
		finish = d.finish4
	case dns.QueryStrategy_USE_IP6:
		finish = d.finish6
	default:
		finish = d.finish4 && d.finish6
	}

	if finish {
		d.queryCallback.access.Lock()
		defer d.queryCallback.access.Unlock()

		if common.Done(d.ctx) {
			return
		}

		newError(server.name, " got answer: ", d.domain, " -> ", queryType, " ", d.ips).AtDebug().WriteToLog(session.ExportIDToError(d.ctx))

		d.queryCallback.response = d
		d.queryCallback.cancel()
		d.cancel()
	}
}

func (c *Client) Type() interface{} {
	return dns.ClientType()
}

func (c *Client) Start() error {
	return nil
}

func (c *Client) Close() error {
	c.cancel()
	for _, server := range c.servers {
		server.transport.Close()
	}
	// TODO: fix domain matcher leak
	c.servers = nil
	c.domainMatcher = nil
	c.hosts = nil
	c.matcherInfos = nil
	c.callbacks = sync.Map{}
	c.cache = sync.Map{}
	return nil
}

func (c *Client) IsOwnLink(ctx context.Context) bool {
	inbound := session.InboundFromContext(ctx)
	return inbound != nil && inbound.Tag == c.tag
}

// old interface

func (c *Client) LookupIP(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(c.ctx, dns.DefaultTimeout)
	defer cancel()
	ips, _, err := c.LookupDefault(ctx, domain)
	return ips, err
}

func (c *Client) LookupIPv4(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(c.ctx, dns.DefaultTimeout)
	defer cancel()
	ips, _, err := c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP4)
	return ips, err
}

func (c *Client) LookupIPv6(domain string) ([]net.IP, error) {
	ctx, cancel := context.WithTimeout(c.ctx, dns.DefaultTimeout)
	defer cancel()
	ips, _, err := c.Lookup(ctx, domain, dns.QueryStrategy_USE_IP6)
	return ips, err
}
