package ca

import (
	"bytes"
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"fmt"
	"strings"
)

// NewFingerprintVerifier returns a function that verifies whether a certificate's SHA-256 fingerprint matches the given one.
func NewFingerprintVerifier(fingerprint string) (func(rawCerts [][]byte, verifiedChains [][]*x509.Certificate) error, error) {
	switch fingerprint {
	case "chrome", "firefox", "safari", "ios", "android", "edge", "360", "qq", "random", "randomized": // WTF???
		return nil, fmt.Errorf("`fingerprint` is used for TLS certificate pinning. If you need to specify the browser fingerprint, use `client-fingerprint`")
	}
	fingerprint = strings.TrimSpace(strings.Replace(fingerprint, ":", "", -1))
	fpByte, err := hex.DecodeString(fingerprint)
	if err != nil {
		return nil, fmt.Errorf("fingerprint string decode error: %w", err)
	}

	if len(fpByte) != 32 {
		return nil, fmt.Errorf("fingerprint string length error,need sha256 fingerprint")
	}

	return func(rawCerts [][]byte, verifiedChains [][]*x509.Certificate) error {
		// ssl pining
		for _, rawCert := range rawCerts {
			hash := sha256.Sum256(rawCert)
			if bytes.Equal(fpByte, hash[:]) {
				return nil
			}
		}
		return errNotMatch
	}, nil
}

// CalculateFingerprint computes the SHA-256 fingerprint of the given DER-encoded certificate and returns it as a hex string.
func CalculateFingerprint(certDER []byte) string {
	hash := sha256.Sum256(certDER)
	return hex.EncodeToString(hash[:])
}
