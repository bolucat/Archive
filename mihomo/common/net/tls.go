package net

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/hex"
	"encoding/pem"
	"fmt"
	"math/big"
)

type Path interface {
	Resolve(path string) string
}

func ParseCert(certificate, privateKey string, path Path) (tls.Certificate, error) {
	if certificate == "" && privateKey == "" {
		var err error
		certificate, privateKey, _, err = NewRandomTLSKeyPair()
		if err != nil {
			return tls.Certificate{}, err
		}
	}
	cert, painTextErr := tls.X509KeyPair([]byte(certificate), []byte(privateKey))
	if painTextErr == nil {
		return cert, nil
	}

	certificate = path.Resolve(certificate)
	privateKey = path.Resolve(privateKey)
	cert, loadErr := tls.LoadX509KeyPair(certificate, privateKey)
	if loadErr != nil {
		return tls.Certificate{}, fmt.Errorf("parse certificate failed, maybe format error:%s, or path error: %s", painTextErr.Error(), loadErr.Error())
	}
	return cert, nil
}

func NewRandomTLSKeyPair() (certificate string, privateKey string, fingerprint string, err error) {
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return
	}
	template := x509.Certificate{SerialNumber: big.NewInt(1)}
	certDER, err := x509.CreateCertificate(
		rand.Reader,
		&template,
		&template,
		&key.PublicKey,
		key)
	if err != nil {
		return
	}
	cert, err := x509.ParseCertificate(certDER)
	if err != nil {
		return
	}
	hash := sha256.Sum256(cert.Raw)
	fingerprint = hex.EncodeToString(hash[:])
	privateKey = string(pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)}))
	certificate = string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: certDER}))
	return
}
