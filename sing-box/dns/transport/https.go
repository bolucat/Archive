package transport

import (
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/dialer"
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

	mDNS "github.com/miekg/dns"
	"golang.org/x/net/http2"
)

const MimeType = "application/dns-message"

var _ adapter.DNSTransport = (*HTTPSTransport)(nil)

func RegisterHTTPS(registry *dns.TransportRegistry) {
	dns.RegisterTransport[option.RemoteHTTPSDNSServerOptions](registry, C.DNSTypeHTTPS, NewHTTPS)
}

type HTTPSTransport struct {
	dns.TransportAdapter
	logger           logger.ContextLogger
	dialer           N.Dialer
	destination      *url.URL
	method           string
	host             string
	queryHeaders     http.Header
	transportAccess  sync.Mutex
	transport        *httpclient.Client
	transportResetAt time.Time
}

func NewHTTPS(ctx context.Context, logger log.ContextLogger, tag string, options option.RemoteHTTPSDNSServerOptions) (adapter.DNSTransport, error) {
	remoteOptions := option.RemoteDNSServerOptions{
		DNSServerAddressOptions: options.DNSServerAddressOptions,
	}
	remoteOptions.DialerOptions = options.DialerOptions
	transportDialer, err := dns.NewRemoteDialer(ctx, remoteOptions)
	if err != nil {
		return nil, err
	}
	tlsOptions := common.PtrValueOrDefault(options.TLS)
	tlsOptions.Enabled = true
	tlsConfig, err := tls.NewClient(ctx, logger, options.Server, tlsOptions)
	if err != nil {
		return nil, err
	}
	if len(tlsConfig.NextProtos()) == 0 {
		tlsConfig.SetNextProtos([]string{http2.NextProtoTLS})
	} else if !common.Contains(tlsConfig.NextProtos(), http2.NextProtoTLS) {
		tlsConfig.SetNextProtos(append([]string{http2.NextProtoTLS}, tlsConfig.NextProtos()...))
	}
	headers := options.Headers.Build()
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
	err = sHTTP.URLSetPath(&destinationURL, path)
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
	httpClientOptions := options.HTTPClientOptions
	return NewHTTPRaw(
		dns.NewTransportAdapterWithRemoteOptions(C.DNSTypeHTTPS, tag, remoteOptions),
		logger,
		transportDialer,
		&destinationURL,
		headers,
		tlsConfig,
		httpClientOptions,
		method,
	)
}

func NewHTTPRaw(
	adapter dns.TransportAdapter,
	logger logger.ContextLogger,
	dialer N.Dialer,
	destination *url.URL,
	headers http.Header,
	tlsConfig tls.Config,
	httpClientOptions option.HTTPClientOptions,
	method string,
) (*HTTPSTransport, error) {
	if destination.Scheme == "https" && tlsConfig == nil {
		return nil, E.New("TLS transport unavailable")
	}
	queryHeaders := headers.Clone()
	if queryHeaders == nil {
		queryHeaders = make(http.Header)
	}
	host := queryHeaders.Get("Host")
	queryHeaders.Del("Host")
	queryHeaders.Set("Accept", MimeType)
	if method == http.MethodPost {
		queryHeaders.Set("Content-Type", MimeType)
	}
	httpClientOptions.Tag = ""
	httpClientOptions.Headers = nil
	currentTransport, err := httpclient.NewClientWithDialer(dialer, tlsConfig, "", httpClientOptions)
	if err != nil {
		return nil, err
	}
	return &HTTPSTransport{
		TransportAdapter: adapter,
		logger:           logger,
		dialer:           dialer,
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
	return dialer.InitializeDetour(t.dialer)
}

func (t *HTTPSTransport) Close() error {
	t.transportAccess.Lock()
	defer t.transportAccess.Unlock()
	t.transport.CloseIdleConnections()
	t.transport = t.transport.Clone()
	return nil
}

func (t *HTTPSTransport) Reset() {
	t.transportAccess.Lock()
	defer t.transportAccess.Unlock()
	t.transport.CloseIdleConnections()
	t.transport = t.transport.Clone()
}

func (t *HTTPSTransport) Exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	startAt := time.Now()
	response, err := t.exchange(ctx, message)
	if err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			t.transportAccess.Lock()
			defer t.transportAccess.Unlock()
			if t.transportResetAt.After(startAt) {
				return nil, err
			}
			t.transport.CloseIdleConnections()
			t.transport = t.transport.Clone()
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
