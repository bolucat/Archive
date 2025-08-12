package encryption

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"errors"
	"math/big"
	"strconv"

	"golang.org/x/crypto/chacha20poly1305"
	"golang.org/x/crypto/hkdf"
)

var MaxNonce = bytes.Repeat([]byte{255}, 12)

func EncodeHeader(b []byte, l int) {
	b[0] = 23
	b[1] = 3
	b[2] = 3
	b[3] = byte(l >> 8)
	b[4] = byte(l)
}

func DecodeHeader(b []byte) (int, error) {
	if b[0] == 23 && b[1] == 3 && b[2] == 3 {
		l := int(b[3])<<8 | int(b[4])
		if l < 17 || l > 17000 { // TODO: TLSv1.3 max length
			return 0, errors.New("invalid length in record's header: " + strconv.Itoa(l))
		}
		return l, nil
	}
	return 0, errors.New("invalid record's header")
}

func NewAead(c byte, secret, salt, info []byte) (aead cipher.AEAD) {
	key := make([]byte, 32)
	hkdf.New(sha256.New, secret, salt, info).Read(key)
	if c&1 == 1 {
		block, _ := aes.NewCipher(key)
		aead, _ = cipher.NewGCM(block)
	} else {
		aead, _ = chacha20poly1305.New(key)
	}
	return
}

func IncreaseNonce(nonce []byte) {
	for i := 0; i < 12; i++ {
		nonce[11-i]++
		if nonce[11-i] != 0 {
			break
		}
	}
}

func randBetween(from int64, to int64) int64 {
	if from == to {
		return from
	}
	bigInt, _ := rand.Int(rand.Reader, big.NewInt(to-from))
	return from + bigInt.Int64()
}
