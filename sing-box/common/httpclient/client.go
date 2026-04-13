package httpclient

import (
	"context"
	"net/http"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/dialer"
	"github.com/sagernet/sing-box/common/tls"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	N "github.com/sagernet/sing/common/network"
)

type Transport struct {
	transport adapter.HTTPTransport
	dialer    N.Dialer
	headers   http.Header
	host      string
	tag       string
}

func NewTransport(ctx context.Context, logger logger.ContextLogger, tag string, options option.HTTPClientOptions) (*Transport, error) {
	rawDialer, err := dialer.NewWithOptions(dialer.Options{
		Context:          ctx,
		Options:          options.DialerOptions,
		RemoteIsDomain:   true,
		DirectResolver:   options.DirectResolver,
		ResolverOnDetour: options.ResolveOnDetour,
		NewDialer:        options.ResolveOnDetour,
		DefaultOutbound:  options.DefaultOutbound,
	})
	if err != nil {
		return nil, err
	}
	switch options.Engine {
	case C.TLSEngineApple:
		transport, transportErr := newAppleTransport(ctx, logger, rawDialer, options)
		if transportErr != nil {
			return nil, transportErr
		}
		headers := options.Headers.Build()
		host := headers.Get("Host")
		headers.Del("Host")
		return &Transport{
			transport: transport,
			dialer:    rawDialer,
			headers:   headers,
			host:      host,
			tag:       tag,
		}, nil
	case C.TLSEngineDefault, "go":
	default:
		return nil, E.New("unknown HTTP engine: ", options.Engine)
	}
	tlsOptions := common.PtrValueOrDefault(options.TLS)
	tlsOptions.Enabled = true
	baseTLSConfig, err := tls.NewClientWithOptions(tls.ClientOptions{
		Context:              ctx,
		Logger:               logger,
		Options:              tlsOptions,
		AllowEmptyServerName: true,
	})
	if err != nil {
		return nil, err
	}
	return NewTransportWithDialer(rawDialer, baseTLSConfig, tag, options)
}

func NewTransportWithDialer(rawDialer N.Dialer, baseTLSConfig tls.Config, tag string, options option.HTTPClientOptions) (*Transport, error) {
	transport, err := newTransport(rawDialer, baseTLSConfig, options)
	if err != nil {
		return nil, err
	}
	headers := options.Headers.Build()
	host := headers.Get("Host")
	headers.Del("Host")
	return &Transport{
		transport: transport,
		dialer:    rawDialer,
		headers:   headers,
		host:      host,
		tag:       tag,
	}, nil
}

func newTransport(rawDialer N.Dialer, baseTLSConfig tls.Config, options option.HTTPClientOptions) (adapter.HTTPTransport, error) {
	version := options.Version
	if version == 0 {
		version = 2
	}
	fallbackDelay := time.Duration(options.DialerOptions.FallbackDelay)
	if fallbackDelay == 0 {
		fallbackDelay = 300 * time.Millisecond
	}
	var transport adapter.HTTPTransport
	var err error
	switch version {
	case 1:
		transport = newHTTP1Transport(rawDialer, baseTLSConfig)
	case 2:
		if options.DisableVersionFallback {
			transport, err = newHTTP2Transport(rawDialer, baseTLSConfig, options.HTTP2Options)
		} else {
			transport, err = newHTTP2FallbackTransport(rawDialer, baseTLSConfig, options.HTTP2Options)
		}
	case 3:
		if baseTLSConfig != nil {
			_, err = baseTLSConfig.STDConfig()
			if err != nil {
				return nil, err
			}
		}
		if options.DisableVersionFallback {
			transport, err = newHTTP3Transport(rawDialer, baseTLSConfig, options.HTTP3Options)
		} else {
			var h2Fallback adapter.HTTPTransport
			h2Fallback, err = newHTTP2FallbackTransport(rawDialer, baseTLSConfig, options.HTTP2Options)
			if err != nil {
				return nil, err
			}
			transport, err = newHTTP3FallbackTransport(rawDialer, baseTLSConfig, h2Fallback, options.HTTP3Options, fallbackDelay)
		}
	default:
		return nil, E.New("unknown HTTP version: ", version)
	}
	if err != nil {
		return nil, err
	}
	return transport, nil
}

func (c *Transport) RoundTrip(request *http.Request) (*http.Response, error) {
	if c.tag == "" && len(c.headers) == 0 && c.host == "" {
		return c.transport.RoundTrip(request)
	}
	if c.tag != "" {
		if transportTag, loaded := transportTagFromContext(request.Context()); loaded && transportTag == c.tag {
			return nil, E.New("HTTP request loopback in transport[", c.tag, "]")
		}
		request = request.Clone(contextWithTransportTag(request.Context(), c.tag))
	} else {
		request = request.Clone(request.Context())
	}
	applyHeaders(request, c.headers, c.host)
	return c.transport.RoundTrip(request)
}

func (c *Transport) CloseIdleConnections() {
	c.transport.CloseIdleConnections()
}

func (c *Transport) Clone() adapter.HTTPTransport {
	return &Transport{
		transport: c.transport.Clone(),
		dialer:    c.dialer,
		headers:   c.headers.Clone(),
		host:      c.host,
		tag:       c.tag,
	}
}

func (c *Transport) Close() error {
	return c.transport.Close()
}

// InitializeDetour eagerly resolves the detour dialer backing transport so that
// detour misconfigurations surface at startup instead of on the first request.
func InitializeDetour(transport adapter.HTTPTransport) error {
	if shared, isShared := transport.(*sharedTransport); isShared {
		transport = shared.HTTPTransport
	}
	inner, isInner := transport.(*Transport)
	if !isInner {
		return nil
	}
	return dialer.InitializeDetour(inner.dialer)
}
