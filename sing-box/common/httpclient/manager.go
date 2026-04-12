package httpclient

import (
	"context"
	"net/http"
	"sync"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
)

var (
	_ adapter.HTTPClientManager = (*Manager)(nil)
	_ adapter.LifecycleService  = (*Manager)(nil)
)

type Manager struct {
	ctx                      context.Context
	logger                   log.ContextLogger
	access                   sync.Mutex
	defines                  map[string]option.HTTPClient
	clients                  map[string]*Client
	defaultTag               string
	defaultTransport         http.RoundTripper
	defaultTransportFallback func() (*Client, error)
	fallbackClient           *Client
}

func NewManager(ctx context.Context, logger log.ContextLogger, clients []option.HTTPClient, defaultHTTPClient string) *Manager {
	defines := make(map[string]option.HTTPClient, len(clients))
	for _, client := range clients {
		defines[client.Tag] = client
	}
	defaultTag := defaultHTTPClient
	if defaultTag == "" && len(clients) > 0 {
		defaultTag = clients[0].Tag
	}
	return &Manager{
		ctx:        ctx,
		logger:     logger,
		defines:    defines,
		clients:    make(map[string]*Client),
		defaultTag: defaultTag,
	}
}

func (m *Manager) Initialize(defaultTransportFallback func() (*Client, error)) {
	m.defaultTransportFallback = defaultTransportFallback
}

func (m *Manager) Name() string {
	return "http-client"
}

func (m *Manager) Start(stage adapter.StartStage) error {
	if stage != adapter.StartStateStart {
		return nil
	}
	if m.defaultTag != "" {
		transport, err := m.resolveShared(m.defaultTag)
		if err != nil {
			return E.Cause(err, "resolve default http client")
		}
		m.defaultTransport = transport
	} else if m.defaultTransportFallback != nil {
		client, err := m.defaultTransportFallback()
		if err != nil {
			return E.Cause(err, "create default http client")
		}
		m.defaultTransport = client
		m.fallbackClient = client
	}
	return nil
}

func (m *Manager) DefaultTransport() http.RoundTripper {
	return m.defaultTransport
}

func (m *Manager) ResolveTransport(logger logger.ContextLogger, options option.HTTPClientOptions) (http.RoundTripper, error) {
	if options.Tag != "" {
		if options.ResolveOnDetour {
			define, loaded := m.defines[options.Tag]
			if !loaded {
				return nil, E.New("http_client not found: ", options.Tag)
			}
			resolvedOptions := define.Options()
			resolvedOptions.ResolveOnDetour = true
			return NewClient(m.ctx, logger, options.Tag, resolvedOptions)
		}
		return m.resolveShared(options.Tag)
	}
	return NewClient(m.ctx, logger, "", options)
}

func (m *Manager) resolveShared(tag string) (http.RoundTripper, error) {
	m.access.Lock()
	defer m.access.Unlock()
	if client, loaded := m.clients[tag]; loaded {
		return client, nil
	}
	define, loaded := m.defines[tag]
	if !loaded {
		return nil, E.New("http_client not found: ", tag)
	}
	client, err := NewClient(m.ctx, m.logger, tag, define.Options())
	if err != nil {
		return nil, E.Cause(err, "create shared http_client[", tag, "]")
	}
	m.clients[tag] = client
	return client, nil
}

func (m *Manager) Close() error {
	m.access.Lock()
	defer m.access.Unlock()
	if m.clients == nil {
		return nil
	}
	var err error
	for _, client := range m.clients {
		err = E.Append(err, client.Close(), func(err error) error {
			return E.Cause(err, "close http client")
		})
	}
	if m.fallbackClient != nil {
		err = E.Append(err, m.fallbackClient.Close(), func(err error) error {
			return E.Cause(err, "close default http client")
		})
	}
	m.clients = nil
	return err
}
