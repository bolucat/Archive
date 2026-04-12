//go:build !with_quic

package httpclient

import (
	"time"

	"github.com/sagernet/sing-box/common/tls"
	"github.com/sagernet/sing-box/option"
	E "github.com/sagernet/sing/common/exceptions"
	N "github.com/sagernet/sing/common/network"
)

func newHTTP3FallbackTransport(
	rawDialer N.Dialer,
	baseTLSConfig tls.Config,
	h2Fallback httpTransport,
	options option.QUICOptions,
	fallbackDelay time.Duration,
) (httpTransport, error) {
	return nil, E.New("HTTP/3 requires building with the with_quic tag")
}

func newHTTP3Transport(
	rawDialer N.Dialer,
	baseTLSConfig tls.Config,
	options option.QUICOptions,
) (httpTransport, error) {
	return nil, E.New("HTTP/3 requires building with the with_quic tag")
}
