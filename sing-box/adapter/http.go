package adapter

import (
	"net/http"

	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common/logger"
)

type HTTPClientManager interface {
	ResolveTransport(logger logger.ContextLogger, options option.HTTPClientOptions) (http.RoundTripper, error)
	DefaultTransport() http.RoundTripper
}
