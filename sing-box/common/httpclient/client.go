package httpclient

import (
	"context"
	"io"
	"net/http"
	"time"

	"github.com/sagernet/sing-box/common/dialer"
	"github.com/sagernet/sing-box/common/tls"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	N "github.com/sagernet/sing/common/network"
)

type httpTransport interface {
	http.RoundTripper
	CloseIdleConnections()
	Clone() httpTransport
}

type Client struct {
	transport httpTransport
	headers   http.Header
	host      string
	tag       string
}

func NewClient(ctx context.Context, logger logger.ContextLogger, tag string, options option.HTTPClientOptions) (*Client, error) {
	rawDialer, err := dialer.NewWithOptions(dialer.Options{
		Context:          ctx,
		Options:          options.DialerOptions,
		RemoteIsDomain:   true,
		ResolverOnDetour: options.ResolveOnDetour,
		NewDialer:        options.ResolveOnDetour,
		DefaultOutbound:  options.DefaultOutbound,
	})
	if err != nil {
		return nil, err
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
	return NewClientWithDialer(rawDialer, baseTLSConfig, tag, options)
}

func NewClientWithDialer(rawDialer N.Dialer, baseTLSConfig tls.Config, tag string, options option.HTTPClientOptions) (*Client, error) {
	headers := options.Headers.Build()
	host := headers.Get("Host")
	headers.Del("Host")
	transport, err := newTransport(rawDialer, baseTLSConfig, options)
	if err != nil {
		return nil, err
	}
	return &Client{
		transport: transport,
		headers:   headers,
		host:      host,
		tag:       tag,
	}, nil
}

func newTransport(rawDialer N.Dialer, baseTLSConfig tls.Config, options option.HTTPClientOptions) (httpTransport, error) {
	version := options.Version
	if version == 0 {
		version = 2
	}
	fallbackDelay := time.Duration(options.DialerOptions.FallbackDelay)
	if fallbackDelay == 0 {
		fallbackDelay = 300 * time.Millisecond
	}
	var transport httpTransport
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
			var h2Fallback httpTransport
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

func (c *Client) RoundTrip(request *http.Request) (*http.Response, error) {
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

func (c *Client) CloseIdleConnections() {
	c.transport.CloseIdleConnections()
}

func (c *Client) Clone() *Client {
	return &Client{
		transport: c.transport.Clone(),
		headers:   c.headers.Clone(),
		host:      c.host,
		tag:       c.tag,
	}
}

func (c *Client) Close() error {
	c.CloseIdleConnections()
	if closer, isCloser := c.transport.(io.Closer); isCloser {
		return closer.Close()
	}
	return nil
}
