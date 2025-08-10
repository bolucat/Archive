package encryption

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"math/big"
	"strconv"

	"golang.org/x/crypto/chacha20poly1305"
)

func encodeHeader(b []byte, l int) {
	b[0] = 23
	b[1] = 3
	b[2] = 3
	b[3] = byte(l >> 8)
	b[4] = byte(l)
}

func decodeHeader(b []byte) (int, error) {
	if b[0] == 23 && b[1] == 3 && b[2] == 3 {
		l := int(b[3])<<8 | int(b[4])
		if l < 17 || l > 17000 { // TODO
			return 0, errors.New("invalid length in record's header: " + strconv.Itoa(l))
		}
		return l, nil
	}
	return 0, errors.New("invalid record's header")
}

func newAead(c byte, k []byte) cipher.AEAD {
	switch c {
	case 0:
		if block, err := aes.NewCipher(k); err == nil {
			aead, _ := cipher.NewGCM(block)
			return aead
		}
	case 1:
		aead, _ := chacha20poly1305.New(k)
		return aead
	}
	return nil
}

func increaseNonce(nonce []byte) {
	for i := 0; i < 12; i++ {
		nonce[11-i]++
		if nonce[11-i] != 0 {
			break
		}
		if i == 11 {
			// TODO
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
