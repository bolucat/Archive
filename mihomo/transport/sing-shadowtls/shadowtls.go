package sing_shadowtls

import (
	"context"
	"crypto/tls"
	"net"

	"github.com/metacubex/mihomo/component/ca"
	tlsC "github.com/metacubex/mihomo/component/tls"
	"github.com/metacubex/mihomo/log"

	"github.com/metacubex/sing-shadowtls"
	utls "github.com/metacubex/utls"
	"golang.org/x/exp/slices"
)

const (
	Mode string = "shadow-tls"
)

var (
	DefaultALPN = []string{"h2", "http/1.1"}
	WsALPN      = []string{"http/1.1"}
)

type ShadowTLSOption struct {
	Password          string
	Host              string
	Fingerprint       string
	ClientFingerprint string
	SkipCertVerify    bool
	Version           int
	ALPN              []string
}

func NewShadowTLS(ctx context.Context, conn net.Conn, option *ShadowTLSOption) (net.Conn, error) {
	tlsConfig := &tls.Config{
		NextProtos:         option.ALPN,
		MinVersion:         tls.VersionTLS12,
		InsecureSkipVerify: option.SkipCertVerify,
		ServerName:         option.Host,
	}
	if option.Version == 1 {
		tlsConfig.MaxVersion = tls.VersionTLS12 // ShadowTLS v1 only support TLS 1.2
	}

	var err error
	tlsConfig, err = ca.GetSpecifiedFingerprintTLSConfig(tlsConfig, option.Fingerprint)
	if err != nil {
		return nil, err
	}

	tlsHandshake := uTLSHandshakeFunc(tlsConfig, option.ClientFingerprint)
	client, err := shadowtls.NewClient(shadowtls.ClientConfig{
		Version:      option.Version,
		Password:     option.Password,
		TLSHandshake: tlsHandshake,
		Logger:       log.SingLogger,
	})
	if err != nil {
		return nil, err
	}
	return client.DialContextConn(ctx, conn)
}

func uTLSHandshakeFunc(config *tls.Config, clientFingerprint string) shadowtls.TLSHandshakeFunc {
	return func(ctx context.Context, conn net.Conn, sessionIDGenerator shadowtls.TLSSessionIDGeneratorFunc) error {
		tlsConfig := tlsC.UConfig(config)
		tlsConfig.SessionIDGenerator = sessionIDGenerator
		clientFingerprint := clientFingerprint
		if tlsC.HaveGlobalFingerprint() && len(clientFingerprint) == 0 {
			clientFingerprint = tlsC.GetGlobalFingerprint()
		}
		if config.MaxVersion == tls.VersionTLS12 { // for ShadowTLS v1
			clientFingerprint = ""
		}
		if len(clientFingerprint) != 0 {
			if fingerprint, exists := tlsC.GetFingerprint(clientFingerprint); exists {
				tlsConn := tlsC.UClient(conn, tlsConfig, fingerprint)
				if slices.Equal(tlsConfig.NextProtos, WsALPN) {
					err := tlsC.BuildWebsocketHandshakeState(tlsConn)
					if err != nil {
						return err
					}
				}
				return tlsConn.HandshakeContext(ctx)
			}
		}
		tlsConn := utls.Client(conn, tlsConfig)
		return tlsConn.HandshakeContext(ctx)
	}
}
