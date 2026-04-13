package transport

import (
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/httpclient"
	"github.com/sagernet/sing-box/common/tls"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/dns"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common"
	"github.com/sagernet/sing/common/buf"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	M "github.com/sagernet/sing/common/metadata"
	N "github.com/sagernet/sing/common/network"
	sHTTP "github.com/sagernet/sing/protocol/http"
	"github.com/sagernet/sing/service"

	mDNS "github.com/miekg/dns"
)

const MimeType = "application/dns-message"

var _ adapter.DNSTransport = (*HTTPSTransport)(nil)

func RegisterHTTPS(registry *dns.TransportRegistry) {
	dns.RegisterTransport[option.RemoteHTTPSDNSServerOptions](registry, C.DNSTypeHTTPS, NewHTTPS)
}

type HTTPSTransport struct {
	dns.TransportAdapter
	logger           logger.ContextLogger
	destination      *url.URL
	method           string
	host             string
	queryHeaders     http.Header
	transportAccess  sync.Mutex
	transport        adapter.HTTPTransport
	transportResetAt time.Time
}

func NewHTTPS(ctx context.Context, logger log.ContextLogger, tag string, options option.RemoteHTTPSDNSServerOptions) (adapter.DNSTransport, error) {
	headers := options.Headers.Build()
	host := headers.Get("Host")
	headers.Del("Host")
	headers.Set("Accept", MimeType)
	serverAddr := options.DNSServerAddressOptions.Build()
	if serverAddr.Port == 0 {
		serverAddr.Port = 443
	}
	if !serverAddr.IsValid() {
		return nil, E.New("invalid server address: ", serverAddr)
	}
	destinationURL := url.URL{
		Scheme: "https",
		Host:   doHURLHost(serverAddr, 443),
	}
	path := options.Path
	if path == "" {
		path = "/dns-query"
	}
	err := sHTTP.URLSetPath(&destinationURL, path)
	if err != nil {
		return nil, err
	}
	method := strings.ToUpper(options.Method)
	if method == "" {
		method = http.MethodPost
	}
	switch method {
	case http.MethodGet, http.MethodPost:
	default:
		return nil, E.New("unsupported HTTPS DNS method: ", options.Method)
	}
	if method == http.MethodPost {
		headers.Set("Content-Type", MimeType)
	}
	httpClientOptions := options.HTTPClientOptions
	tlsOptions := common.PtrValueOrDefault(httpClientOptions.TLS)
	tlsOptions.Enabled = true
	httpClientOptions.TLS = &tlsOptions
	httpClientOptions.Tag = ""
	httpClientOptions.Headers = nil
	if options.ServerIsDomain() {
		httpClientOptions.DirectResolver = true
	}
	httpClientManager := service.FromContext[adapter.HTTPClientManager](ctx)
	transport, err := httpClientManager.ResolveTransport(ctx, logger, httpClientOptions)
	if err != nil {
		return nil, err
	}
	remoteOptions := option.RemoteDNSServerOptions{
		RawLocalDNSServerOptions: option.RawLocalDNSServerOptions{
			DialerOptions: options.DialerOptions,
		},
		DNSServerAddressOptions: options.DNSServerAddressOptions,
	}
	return &HTTPSTransport{
		TransportAdapter: dns.NewTransportAdapterWithRemoteOptions(C.DNSTypeHTTPS, tag, remoteOptions),
		logger:           logger,
		destination:      &destinationURL,
		method:           method,
		host:             host,
		queryHeaders:     headers,
		transport:        transport,
	}, nil
}

func NewHTTPRaw(
	adapter dns.TransportAdapter,
	logger logger.ContextLogger,
	dialer N.Dialer,
	destination *url.URL,
	headers http.Header,
	tlsConfig tls.Config,
	method string,
) (*HTTPSTransport, error) {
	if destination.Scheme == "https" && tlsConfig == nil {
		return nil, E.New("TLS transport unavailable")
	}
	queryHeaders := headers.Clone()
	host := queryHeaders.Get("Host")
	queryHeaders.Del("Host")
	queryHeaders.Set("Accept", MimeType)
	if method == http.MethodPost {
		queryHeaders.Set("Content-Type", MimeType)
	}
	currentTransport, err := httpclient.NewTransportWithDialer(dialer, tlsConfig, "", option.HTTPClientOptions{})
	if err != nil {
		return nil, err
	}
	return &HTTPSTransport{
		TransportAdapter: adapter,
		logger:           logger,
		destination:      destination,
		method:           method,
		host:             host,
		queryHeaders:     queryHeaders,
		transport:        currentTransport,
	}, nil
}

func (t *HTTPSTransport) Start(stage adapter.StartStage) error {
	if stage != adapter.StartStateStart {
		return nil
	}
	return httpclient.InitializeDetour(t.transport)
}

func (t *HTTPSTransport) Close() error {
	t.transportAccess.Lock()
	defer t.transportAccess.Unlock()
	if t.transport == nil {
		return nil
	}
	err := t.transport.Close()
	t.transport = nil
	return err
}

func (t *HTTPSTransport) Reset() {
	t.transportAccess.Lock()
	defer t.transportAccess.Unlock()
	if t.transport == nil {
		return
	}
	oldTransport := t.transport
	oldTransport.CloseIdleConnections()
	// Close is intentionally avoided here because some Clone implementations share transport state.
	t.transport = oldTransport.Clone()
}

func (t *HTTPSTransport) Exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	startAt := time.Now()
	response, err := t.exchange(ctx, message)
	if err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			t.transportAccess.Lock()
			defer t.transportAccess.Unlock()
			if t.transport == nil || t.transportResetAt.After(startAt) {
				return nil, err
			}
			oldTransport := t.transport
			oldTransport.CloseIdleConnections()
			t.transport = oldTransport.Clone()
			t.transportResetAt = time.Now()
		}
		return nil, err
	}
	return response, nil
}

func (t *HTTPSTransport) exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	exMessage := *message
	exMessage.Id = 0
	exMessage.Compress = true
	requestBuffer := buf.NewSize(1 + message.Len())
	rawMessage, err := exMessage.PackBuffer(requestBuffer.FreeBytes())
	if err != nil {
		requestBuffer.Release()
		return nil, err
	}
	requestURL := *t.destination
	var request *http.Request
	switch t.method {
	case http.MethodGet:
		query := requestURL.Query()
		query.Set("dns", base64.RawURLEncoding.EncodeToString(rawMessage))
		requestURL.RawQuery = query.Encode()
		request, err = http.NewRequestWithContext(ctx, http.MethodGet, requestURL.String(), nil)
	default:
		request, err = http.NewRequestWithContext(ctx, http.MethodPost, requestURL.String(), bytes.NewReader(rawMessage))
	}
	if err != nil {
		requestBuffer.Release()
		return nil, err
	}
	request.Header = t.queryHeaders.Clone()
	if t.host != "" {
		request.Host = t.host
	}
	t.transportAccess.Lock()
	currentTransport := t.transport
	t.transportAccess.Unlock()
	if currentTransport == nil {
		requestBuffer.Release()
		return nil, net.ErrClosed
	}
	response, err := currentTransport.RoundTrip(request)
	requestBuffer.Release()
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, E.New("unexpected status: ", response.Status)
	}
	var responseMessage mDNS.Msg
	if response.ContentLength > 0 {
		responseBuffer := buf.NewSize(int(response.ContentLength))
		defer responseBuffer.Release()
		_, err = responseBuffer.ReadFullFrom(response.Body, int(response.ContentLength))
		if err != nil {
			return nil, err
		}
		err = responseMessage.Unpack(responseBuffer.Bytes())
	} else {
		rawMessage, err = io.ReadAll(response.Body)
		if err != nil {
			return nil, err
		}
		err = responseMessage.Unpack(rawMessage)
	}
	if err != nil {
		return nil, err
	}
	return &responseMessage, nil
}

func doHURLHost(serverAddr M.Socksaddr, defaultPort uint16) string {
	if serverAddr.Port != defaultPort {
		return serverAddr.String()
	}
	if serverAddr.IsIPv6() {
		return "[" + serverAddr.AddrString() + "]"
	}
	return serverAddr.AddrString()
}
