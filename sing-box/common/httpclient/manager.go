package httpclient

import (
	"context"
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
	transports               map[string]*Transport
	defaultTag               string
	defaultTransport         adapter.HTTPTransport
	defaultTransportFallback func() (*Transport, error)
	fallbackTransport        *Transport
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
		transports: make(map[string]*Transport),
		defaultTag: defaultTag,
	}
}

func (m *Manager) Initialize(defaultTransportFallback func() (*Transport, error)) {
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
		transport, err := m.defaultTransportFallback()
		if err != nil {
			return E.Cause(err, "create default http client")
		}
		m.defaultTransport = transport
		m.fallbackTransport = transport
	}
	return nil
}

func (m *Manager) DefaultTransport() adapter.HTTPTransport {
	if m.defaultTransport == nil {
		return nil
	}
	return &sharedTransport{m.defaultTransport}
}

func (m *Manager) ResolveTransport(ctx context.Context, logger logger.ContextLogger, options option.HTTPClientOptions) (adapter.HTTPTransport, error) {
	if options.Tag != "" {
		if options.ResolveOnDetour {
			define, loaded := m.defines[options.Tag]
			if !loaded {
				return nil, E.New("http_client not found: ", options.Tag)
			}
			resolvedOptions := define.Options()
			resolvedOptions.ResolveOnDetour = true
			return NewTransport(ctx, logger, options.Tag, resolvedOptions)
		}
		transport, err := m.resolveShared(options.Tag)
		if err != nil {
			return nil, err
		}
		return &sharedTransport{transport}, nil
	}
	return NewTransport(ctx, logger, "", options)
}

func (m *Manager) resolveShared(tag string) (adapter.HTTPTransport, error) {
	m.access.Lock()
	defer m.access.Unlock()
	if transport, loaded := m.transports[tag]; loaded {
		return transport, nil
	}
	define, loaded := m.defines[tag]
	if !loaded {
		return nil, E.New("http_client not found: ", tag)
	}
	transport, err := NewTransport(m.ctx, m.logger, tag, define.Options())
	if err != nil {
		return nil, E.Cause(err, "create shared http_client[", tag, "]")
	}
	m.transports[tag] = transport
	return transport, nil
}

type sharedTransport struct {
	adapter.HTTPTransport
}

func (t *sharedTransport) CloseIdleConnections() {
}

func (t *sharedTransport) Close() error {
	return nil
}

func (m *Manager) ResetNetwork() {
	m.access.Lock()
	defer m.access.Unlock()
	for _, transport := range m.transports {
		transport.CloseIdleConnections()
	}
	if m.fallbackTransport != nil {
		m.fallbackTransport.CloseIdleConnections()
	}
}

func (m *Manager) Close() error {
	m.access.Lock()
	defer m.access.Unlock()
	if m.transports == nil {
		return nil
	}
	var err error
	for _, transport := range m.transports {
		err = E.Append(err, transport.Close(), func(err error) error {
			return E.Cause(err, "close http client")
		})
	}
	if m.fallbackTransport != nil {
		err = E.Append(err, m.fallbackTransport.Close(), func(err error) error {
			return E.Cause(err, "close default http client")
		})
	}
	m.transports = nil
	return err
}
