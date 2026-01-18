package main

import (
	"crypto/tls"
	"fmt"
	"net/url"
	"os"
	"runtime"
	"time"

	"github.com/NodePassProject/logs"
	"github.com/NodePassProject/nodepass/internal"
)

func start(args []string) error {
	if len(args) != 2 {
		return fmt.Errorf("start: empty URL command")
	}

	parsedURL, err := url.Parse(args[1])
	if err != nil {
		return fmt.Errorf("start: parse URL failed: %w", err)
	}

	logger := initLogger(parsedURL.Query().Get("log"))

	core, err := createCore(parsedURL, logger)
	if err != nil {
		return fmt.Errorf("start: create core failed: %w", err)
	}

	core.Run()
	return nil
}

func initLogger(level string) *logs.Logger {
	logger := logs.NewLogger(logs.Info, true)
	switch level {
	case "none":
		logger.SetLogLevel(logs.None)
	case "debug":
		logger.SetLogLevel(logs.Debug)
		logger.Debug("Init log level: DEBUG")
	case "warn":
		logger.SetLogLevel(logs.Warn)
		logger.Warn("Init log level: WARN")
	case "error":
		logger.SetLogLevel(logs.Error)
		logger.Error("Init log level: ERROR")
	case "event":
		logger.SetLogLevel(logs.Event)
		logger.Event("Init log level: EVENT")
	default:
	}
	return logger
}

func createCore(parsedURL *url.URL, logger *logs.Logger) (interface{ Run() }, error) {
	switch parsedURL.Scheme {
	case "server":
		tlsCode, tlsConfig := getTLSProtocol(parsedURL, logger)
		return internal.NewServer(parsedURL, tlsCode, tlsConfig, logger)
	case "client":
		return internal.NewClient(parsedURL, logger)
	case "master":
		tlsCode, tlsConfig := getTLSProtocol(parsedURL, logger)
		return internal.NewMaster(parsedURL, tlsCode, tlsConfig, logger, version)
	default:
		return nil, fmt.Errorf("createCore: unknown core: %v", parsedURL)
	}
}

func getTLSProtocol(parsedURL *url.URL, logger *logs.Logger) (string, *tls.Config) {
	tlsConfig, err := internal.NewTLSConfig()
	if err != nil {
		logger.Error("Generate TLS config failed: %v", err)
		logger.Warn("TLS code-0: nil cert")
		return "0", nil
	}

	tlsConfig.MinVersion = tls.VersionTLS13

	switch parsedURL.Query().Get("tls") {
	case "1":
		logger.Info("TLS code-1: RAM cert with TLS 1.3")
		return "1", tlsConfig
	case "2":
		crtFile, keyFile := parsedURL.Query().Get("crt"), parsedURL.Query().Get("key")
		cert, err := tls.LoadX509KeyPair(crtFile, keyFile)
		if err != nil {
			logger.Error("Certificate load failed: %v", err)
			logger.Warn("TLS code-1: RAM cert with TLS 1.3")
			return "1", tlsConfig
		}

		cachedCert := cert
		lastReload := time.Now()
		tlsConfig = &tls.Config{
			MinVersion: tls.VersionTLS13,
			GetCertificate: func(clientHello *tls.ClientHelloInfo) (*tls.Certificate, error) {
				if time.Since(lastReload) >= internal.ReloadInterval {
					newCert, err := tls.LoadX509KeyPair(crtFile, keyFile)
					if err != nil {
						logger.Error("Certificate reload failed: %v", err)
					} else {
						logger.Debug("TLS cert reloaded: %v", crtFile)
						cachedCert = newCert
					}
					lastReload = time.Now()
				}
				return &cachedCert, nil
			},
		}

		if cert.Leaf != nil {
			logger.Info("TLS code-2: %v with TLS 1.3", cert.Leaf.Subject.CommonName)
		} else {
			logger.Warn("TLS code-2: unknown cert name with TLS 1.3")
		}
		return "2", tlsConfig
	default:
		if poolType := parsedURL.Query().Get("type"); poolType == "1" || poolType == "3" {
			logger.Info("TLS code-1: RAM cert with TLS 1.3 for stream pool")
			return "1", tlsConfig
		}
		logger.Warn("TLS code-0: unencrypted")
		return "0", nil
	}
}

func exit(err error) {
	errMsg := "none"
	if err != nil {
		errMsg = err.Error()
	}
	fmt.Fprintf(os.Stderr,
		"nodepass-%s %s/%s pid=%d error=%s\nvisit https://github.com/NodePassProject for more information\n",
		version, runtime.GOOS, runtime.GOARCH, os.Getpid(), errMsg)

	os.Exit(1)
}
