package ca

import (
	"crypto/tls"
	"crypto/x509"
	_ "embed"
	"errors"
	"fmt"
	"os"
	"strconv"
	"sync"

	C "github.com/metacubex/mihomo/constant"
)

var globalCertPool *x509.CertPool
var mutex sync.RWMutex
var errNotMatch = errors.New("certificate fingerprints do not match")

//go:embed ca-certificates.crt
var _CaCertificates []byte
var DisableEmbedCa, _ = strconv.ParseBool(os.Getenv("DISABLE_EMBED_CA"))
var DisableSystemCa, _ = strconv.ParseBool(os.Getenv("DISABLE_SYSTEM_CA"))

func AddCertificate(certificate string) error {
	mutex.Lock()
	defer mutex.Unlock()

	if certificate == "" {
		return fmt.Errorf("certificate is empty")
	}

	if globalCertPool == nil {
		initializeCertPool()
	}

	if globalCertPool.AppendCertsFromPEM([]byte(certificate)) {
		return nil
	} else if cert, err := x509.ParseCertificate([]byte(certificate)); err == nil {
		globalCertPool.AddCert(cert)
		return nil
	} else {
		return fmt.Errorf("add certificate failed")
	}
}

func initializeCertPool() {
	var err error
	if DisableSystemCa {
		globalCertPool = x509.NewCertPool()
	} else {
		globalCertPool, err = x509.SystemCertPool()
		if err != nil {
			globalCertPool = x509.NewCertPool()
		}
	}
	if !DisableEmbedCa {
		globalCertPool.AppendCertsFromPEM(_CaCertificates)
	}
}

func ResetCertificate() {
	mutex.Lock()
	defer mutex.Unlock()
	initializeCertPool()
}

func getCertPool() *x509.CertPool {
	if globalCertPool == nil {
		mutex.Lock()
		defer mutex.Unlock()
		if globalCertPool != nil {
			return globalCertPool
		}
		initializeCertPool()
	}
	return globalCertPool
}

func GetCertPool(customCA string, customCAString string) (*x509.CertPool, error) {
	var certificate []byte
	var err error
	if len(customCA) > 0 {
		path := C.Path.Resolve(customCA)
		if !C.Path.IsSafePath(path) {
			return nil, C.Path.ErrNotSafePath(path)
		}
		certificate, err = os.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("load ca error: %w", err)
		}
	} else if customCAString != "" {
		certificate = []byte(customCAString)
	}
	if len(certificate) > 0 {
		certPool := x509.NewCertPool()
		if !certPool.AppendCertsFromPEM(certificate) {
			return nil, fmt.Errorf("failed to parse certificate:\n\n %s", certificate)
		}
		return certPool, nil
	} else {
		return getCertPool(), nil
	}
}

// GetTLSConfig specified fingerprint, customCA and customCAString
func GetTLSConfig(tlsConfig *tls.Config, fingerprint string, customCA string, customCAString string) (_ *tls.Config, err error) {
	if tlsConfig == nil {
		tlsConfig = &tls.Config{}
	}
	tlsConfig.RootCAs, err = GetCertPool(customCA, customCAString)
	if err != nil {
		return nil, err
	}

	if len(fingerprint) > 0 {
		tlsConfig.VerifyPeerCertificate, err = NewFingerprintVerifier(fingerprint)
		if err != nil {
			return nil, err
		}
		tlsConfig.InsecureSkipVerify = true
	}
	return tlsConfig, nil
}

// GetSpecifiedFingerprintTLSConfig specified fingerprint
func GetSpecifiedFingerprintTLSConfig(tlsConfig *tls.Config, fingerprint string) (*tls.Config, error) {
	return GetTLSConfig(tlsConfig, fingerprint, "", "")
}

func GetGlobalTLSConfig(tlsConfig *tls.Config) *tls.Config {
	tlsConfig, _ = GetTLSConfig(tlsConfig, "", "", "")
	return tlsConfig
}
