package tls

import (
	"crypto/tls"
	"net"

	"github.com/metacubex/mihomo/common/utils"
	"github.com/metacubex/mihomo/log"

	utls "github.com/metacubex/utls"
	"github.com/mroth/weightedrand/v2"
)

type UConn = utls.UConn

const VersionTLS13 = utls.VersionTLS13

type UClientHelloID struct {
	*utls.ClientHelloID
}

var initRandomFingerprint UClientHelloID
var initUtlsClient string

func UClient(c net.Conn, config *utls.Config, fingerprint UClientHelloID) *UConn {
	return utls.UClient(c, config, *fingerprint.ClientHelloID)
}

func GetFingerprint(ClientFingerprint string) (UClientHelloID, bool) {
	if ClientFingerprint == "none" {
		return UClientHelloID{}, false
	}

	if initRandomFingerprint.ClientHelloID == nil {
		initRandomFingerprint, _ = RollFingerprint()
	}

	if ClientFingerprint == "random" {
		log.Debugln("use initial random HelloID:%s", initRandomFingerprint.Client)
		return initRandomFingerprint, true
	}

	fingerprint, ok := Fingerprints[ClientFingerprint]
	if ok {
		log.Debugln("use specified fingerprint:%s", fingerprint.Client)
		return fingerprint, ok
	} else {
		log.Warnln("wrong ClientFingerprint:%s", ClientFingerprint)
		return UClientHelloID{}, false
	}
}

func RollFingerprint() (UClientHelloID, bool) {
	chooser, _ := weightedrand.NewChooser(
		weightedrand.NewChoice("chrome", 6),
		weightedrand.NewChoice("safari", 3),
		weightedrand.NewChoice("ios", 2),
		weightedrand.NewChoice("firefox", 1),
	)
	initClient := chooser.Pick()
	log.Debugln("initial random HelloID:%s", initClient)
	fingerprint, ok := Fingerprints[initClient]
	return fingerprint, ok
}

var Fingerprints = map[string]UClientHelloID{
	"chrome":                     {&utls.HelloChrome_Auto},
	"chrome_psk":                 {&utls.HelloChrome_100_PSK},
	"chrome_psk_shuffle":         {&utls.HelloChrome_106_Shuffle},
	"chrome_padding_psk_shuffle": {&utls.HelloChrome_114_Padding_PSK_Shuf},
	"chrome_pq":                  {&utls.HelloChrome_115_PQ},
	"chrome_pq_psk":              {&utls.HelloChrome_115_PQ_PSK},
	"firefox":                    {&utls.HelloFirefox_Auto},
	"safari":                     {&utls.HelloSafari_Auto},
	"ios":                        {&utls.HelloIOS_Auto},
	"android":                    {&utls.HelloAndroid_11_OkHttp},
	"edge":                       {&utls.HelloEdge_Auto},
	"360":                        {&utls.Hello360_Auto},
	"qq":                         {&utls.HelloQQ_Auto},
	"random":                     {nil},
	"randomized":                 {nil},
}

func init() {
	weights := utls.DefaultWeights
	weights.TLSVersMax_Set_VersionTLS13 = 1
	weights.FirstKeyShare_Set_CurveP256 = 0
	randomized := utls.HelloRandomized
	randomized.Seed, _ = utls.NewPRNGSeed()
	randomized.Weights = &weights
	Fingerprints["randomized"] = UClientHelloID{&randomized}
}

func UCertificates(it tls.Certificate) utls.Certificate {
	return utls.Certificate{
		Certificate: it.Certificate,
		PrivateKey:  it.PrivateKey,
		SupportedSignatureAlgorithms: utils.Map(it.SupportedSignatureAlgorithms, func(it tls.SignatureScheme) utls.SignatureScheme {
			return utls.SignatureScheme(it)
		}),
		OCSPStaple:                  it.OCSPStaple,
		SignedCertificateTimestamps: it.SignedCertificateTimestamps,
		Leaf:                        it.Leaf,
	}
}

func UConfig(config *tls.Config) *utls.Config {
	return &utls.Config{
		Rand:                  config.Rand,
		Time:                  config.Time,
		Certificates:          utils.Map(config.Certificates, UCertificates),
		VerifyPeerCertificate: config.VerifyPeerCertificate,
		RootCAs:               config.RootCAs,
		NextProtos:            config.NextProtos,
		ServerName:            config.ServerName,
		InsecureSkipVerify:    config.InsecureSkipVerify,
		CipherSuites:          config.CipherSuites,
		MinVersion:            config.MinVersion,
		MaxVersion:            config.MaxVersion,
		CurvePreferences: utils.Map(config.CurvePreferences, func(it tls.CurveID) utls.CurveID {
			return utls.CurveID(it)
		}),
		SessionTicketsDisabled: config.SessionTicketsDisabled,
		Renegotiation:          utls.RenegotiationSupport(config.Renegotiation),
	}
}

// BuildWebsocketHandshakeState it will only send http/1.1 in its ALPN.
// Copy from https://github.com/XTLS/Xray-core/blob/main/transport/internet/tls/tls.go
func BuildWebsocketHandshakeState(c *UConn) error {
	// Build the handshake state. This will apply every variable of the TLS of the
	// fingerprint in the UConn
	if err := c.BuildHandshakeState(); err != nil {
		return err
	}
	// Iterate over extensions and check for utls.ALPNExtension
	hasALPNExtension := false
	for _, extension := range c.Extensions {
		if alpn, ok := extension.(*utls.ALPNExtension); ok {
			hasALPNExtension = true
			alpn.AlpnProtocols = []string{"http/1.1"}
			break
		}
	}
	if !hasALPNExtension { // Append extension if doesn't exists
		c.Extensions = append(c.Extensions, &utls.ALPNExtension{AlpnProtocols: []string{"http/1.1"}})
	}
	// Rebuild the client hello
	if err := c.BuildHandshakeState(); err != nil {
		return err
	}
	return nil
}

func SetGlobalUtlsClient(Client string) {
	initUtlsClient = Client
}

func HaveGlobalFingerprint() bool {
	return len(initUtlsClient) != 0 && initUtlsClient != "none"
}

func GetGlobalFingerprint() string {
	return initUtlsClient
}
