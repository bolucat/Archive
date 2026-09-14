package utils

import (
	"crypto/ecdh"
	"crypto/rand"
	"encoding/binary"
	"encoding/pem"
	"errors"
	"fmt"
	"net"
	"strings"
)

// ECHKeyOptions describes a single ECH config. Keys use X25519 and HKDF-SHA256
// for compatibility with Hysteria clients, including older Go TLS versions.
type ECHKeyOptions struct {
	PublicName    string
	ConfigID      *uint8 // nil selects a cryptographically random ID
	MaxNameLength uint8  // padding hint, not a limit on the inner server name
	AEADs         []string
}

// GenerateECHKeys returns a private PEM file containing ECH KEYS and ECH
// CONFIGS blocks, and the public ECHConfigList for clients. The wire encoding
// follows RFC 9849, Section 4; the PEM container matches LoadECHKeys.
func GenerateECHKeys(options ECHKeyOptions) (keyPEM, configList []byte, err error) {
	if err := validateECHPublicName(options.PublicName); err != nil {
		return nil, nil, err
	}
	aeads := options.AEADs
	if aeads == nil {
		aeads = []string{"aes-128-gcm"}
	}
	if len(aeads) == 0 {
		return nil, nil, errors.New("at least one AEAD is required")
	}
	var suites []byte
	seen := make(map[uint16]bool)
	for _, name := range aeads {
		var id uint16
		switch name {
		case "aes-128-gcm":
			id = 1
		case "aes-256-gcm":
			id = 2
		case "chacha20-poly1305":
			id = 3
		default:
			return nil, nil, fmt.Errorf("unsupported AEAD %q (use aes-128-gcm, aes-256-gcm, or chacha20-poly1305)", name)
		}
		if seen[id] {
			return nil, nil, fmt.Errorf("duplicate AEAD %q", name)
		}
		seen[id] = true
		suites = binary.BigEndian.AppendUint16(suites, 1) // HKDF-SHA256
		suites = binary.BigEndian.AppendUint16(suites, id)
	}
	var configID uint8
	if options.ConfigID != nil {
		configID = *options.ConfigID
	} else {
		var b [1]byte
		if _, err := rand.Read(b[:]); err != nil {
			return nil, nil, err
		}
		configID = b[0]
	}
	privateKey, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		return nil, nil, err
	}
	publicKey := privateKey.PublicKey().Bytes()
	contents := []byte{configID}
	contents = binary.BigEndian.AppendUint16(contents, 0x0020) // DHKEM(X25519, HKDF-SHA256)
	contents = appendECHVector(contents, publicKey)
	contents = appendECHVector(contents, suites)
	contents = append(contents, options.MaxNameLength, uint8(len(options.PublicName)))
	contents = append(contents, options.PublicName...)
	contents = binary.BigEndian.AppendUint16(contents, 0) // no extensions
	config := binary.BigEndian.AppendUint16(nil, 0xfe0d)
	config = appendECHVector(config, contents)
	configList = appendECHVector(nil, config)
	keyBlob := appendECHVector(nil, privateKey.Bytes())
	keyBlob = appendECHVector(keyBlob, config)
	keyPEM = pem.EncodeToMemory(&pem.Block{Type: pemBlockECHKeys, Bytes: keyBlob})
	keyPEM = append(keyPEM, pem.EncodeToMemory(&pem.Block{Type: pemBlockECHConfigs, Bytes: configList})...)
	return keyPEM, configList, nil
}

// All callers bound their input sizes to well below the uint16 wire limit.
func appendECHVector(dst, value []byte) []byte {
	dst = binary.BigEndian.AppendUint16(dst, uint16(len(value)))
	return append(dst, value...)
}

func validateECHPublicName(name string) error {
	if len(name) == 0 || len(name) > 253 || net.ParseIP(name) != nil || !strings.Contains(name, ".") {
		return errors.New("public-name must be a DNS name such as public.example.com, without a scheme, port, or trailing dot")
	}
	labels := strings.Split(name, ".")
	if strings.Trim(labels[len(labels)-1], "0123456789") == "" {
		return errors.New("public-name must not have an entirely numeric final DNS label")
	}
	for _, label := range labels {
		if len(label) == 0 || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
			return fmt.Errorf("invalid DNS label in public-name %q", name)
		}
		for _, c := range label {
			if !((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '-') {
				return errors.New("public-name must contain only DNS labels (use ASCII or punycode for internationalized names)")
			}
		}
	}
	return nil
}
