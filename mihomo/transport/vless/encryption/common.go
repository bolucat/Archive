package encryption

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"errors"
	"fmt"
	"io"
	"net"
	"time"

	"github.com/metacubex/mihomo/common/pool"

	"github.com/metacubex/blake3"
	"github.com/metacubex/randv2"
)

type CommonConn struct {
	net.Conn
	Client      *ClientInstance
	UnitedKey   []byte
	PreWrite    []byte
	GCM         *GCM
	PeerPadding []byte
	rawInput    bytes.Buffer // PeerInBytes
	PeerGCM     *GCM
	input       bytes.Reader // PeerCache
}

func NewCommonConn(conn net.Conn) *CommonConn {
	return &CommonConn{
		Conn: conn,
	}
}

func (c *CommonConn) Write(b []byte) (int, error) {
	if len(b) == 0 {
		return 0, nil
	}
	outBytes := pool.Get(5 + 8192 + 16)
	defer pool.Put(outBytes)
	for n := 0; n < len(b); {
		b := b[n:]
		if len(b) > 8192 {
			b = b[:8192] // for avoiding another copy() in peer's Read()
		}
		n += len(b)
		headerAndData := outBytes[:5+len(b)+16]
		EncodeHeader(headerAndData, len(b)+16)
		max := false
		if bytes.Equal(c.GCM.Nonce[:], MaxNonce) {
			max = true
		}
		c.GCM.Seal(headerAndData[:5], nil, b, headerAndData[:5])
		if max {
			c.GCM = NewGCM(headerAndData, c.UnitedKey)
		}
		if c.PreWrite != nil {
			headerAndData = append(c.PreWrite, headerAndData...)
			c.PreWrite = nil
		}
		if _, err := c.Conn.Write(headerAndData); err != nil {
			return 0, err
		}
	}
	return len(b), nil
}

func (c *CommonConn) Read(b []byte) (int, error) {
	if len(b) == 0 {
		return 0, nil
	}
	if c.PeerGCM == nil { // client's 0-RTT
		serverRandom := make([]byte, 16)
		if _, err := io.ReadFull(c.Conn, serverRandom); err != nil {
			return 0, err
		}
		c.PeerGCM = NewGCM(serverRandom, c.UnitedKey)
		if xorConn, ok := c.Conn.(*XorConn); ok {
			xorConn.PeerCTR = NewCTR(c.UnitedKey, serverRandom)
		}
	}
	if c.PeerPadding != nil { // client's 1-RTT
		if _, err := io.ReadFull(c.Conn, c.PeerPadding); err != nil {
			return 0, err
		}
		if _, err := c.PeerGCM.Open(c.PeerPadding[:0], nil, c.PeerPadding, nil); err != nil {
			return 0, err
		}
		c.PeerPadding = nil
	}
	if c.input.Len() > 0 {
		return c.input.Read(b)
	}
	peerHeader := make([]byte, 5)
	if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
		return 0, err
	}
	l, err := DecodeHeader(peerHeader) // l: 17~17000
	if err != nil {
		if c.Client != nil && errors.Is(err, ErrInvalidHeader) { // client's 0-RTT
			c.Client.RWLock.Lock()
			if bytes.HasPrefix(c.UnitedKey, c.Client.PfsKey) {
				c.Client.Expire = time.Now() // expired
			}
			c.Client.RWLock.Unlock()
			return 0, errors.New("new handshake needed")
		}
		return 0, err
	}
	c.Client = nil
	c.rawInput.Grow(l)
	peerData := c.rawInput.Bytes()[:l]
	if _, err := io.ReadFull(c.Conn, peerData); err != nil {
		return 0, err
	}
	dst := peerData[:l-16]
	if len(dst) <= len(b) {
		dst = b[:len(dst)] // avoids another copy()
	}
	var newGCM *GCM
	if bytes.Equal(c.PeerGCM.Nonce[:], MaxNonce) {
		newGCM = NewGCM(append(peerHeader, peerData...), c.UnitedKey)
	}
	_, err = c.PeerGCM.Open(dst[:0], nil, peerData, peerHeader)
	if newGCM != nil {
		c.PeerGCM = newGCM
	}
	if err != nil {
		return 0, err
	}
	if len(dst) > len(b) {
		c.input.Reset(dst[copy(b, dst):])
		dst = b // for len(dst)
	}
	return len(dst), nil
}

type GCM struct {
	cipher.AEAD
	Nonce [12]byte
}

func NewGCM(ctx, key []byte) *GCM {
	k := make([]byte, 32)
	blake3.DeriveKey(k, string(ctx), key)
	block, _ := aes.NewCipher(k)
	aead, _ := cipher.NewGCM(block)
	return &GCM{AEAD: aead}
	//chacha20poly1305.New()
}

func (a *GCM) Seal(dst, nonce, plaintext, additionalData []byte) []byte {
	if nonce == nil {
		nonce = IncreaseNonce(a.Nonce[:])
	}
	return a.AEAD.Seal(dst, nonce, plaintext, additionalData)
}

func (a *GCM) Open(dst, nonce, ciphertext, additionalData []byte) ([]byte, error) {
	if nonce == nil {
		nonce = IncreaseNonce(a.Nonce[:])
	}
	return a.AEAD.Open(dst, nonce, ciphertext, additionalData)
}

func IncreaseNonce(nonce []byte) []byte {
	for i := 0; i < 12; i++ {
		nonce[11-i]++
		if nonce[11-i] != 0 {
			break
		}
	}
	return nonce
}

var MaxNonce = bytes.Repeat([]byte{255}, 12)

func EncodeLength(l int) []byte {
	return []byte{byte(l >> 8), byte(l)}
}

func DecodeLength(b []byte) int {
	return int(b[0])<<8 | int(b[1])
}

func EncodeHeader(h []byte, l int) {
	h[0] = 23
	h[1] = 3
	h[2] = 3
	h[3] = byte(l >> 8)
	h[4] = byte(l)
}

var ErrInvalidHeader = errors.New("invalid header")

func DecodeHeader(h []byte) (l int, err error) {
	l = int(h[3])<<8 | int(h[4])
	if h[0] != 23 || h[1] != 3 || h[2] != 3 {
		l = 0
	}
	if l < 17 || l > 17000 { // TODO: TLSv1.3 max length
		err = fmt.Errorf("%w: %v", ErrInvalidHeader, h[:5]) // DO NOT CHANGE: relied by client's Read()
	}
	return
}

func randBetween(from int64, to int64) int64 {
	if from == to {
		return from
	}
	return from + randv2.Int64N(to-from)
}
