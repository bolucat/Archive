package encryption

import (
	"bytes"
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"fmt"
	"io"
	"net"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/metacubex/utls/mlkem"
	"golang.org/x/crypto/sha3"
	"golang.org/x/sys/cpu"
)

var (
	// Keep in sync with crypto/tls/cipher_suites.go.
	hasGCMAsmAMD64 = cpu.X86.HasAES && cpu.X86.HasPCLMULQDQ && cpu.X86.HasSSE41 && cpu.X86.HasSSSE3
	hasGCMAsmARM64 = cpu.ARM64.HasAES && cpu.ARM64.HasPMULL
	hasGCMAsmS390X = cpu.S390X.HasAES && cpu.S390X.HasAESCTR && cpu.S390X.HasGHASH
	hasGCMAsmPPC64 = runtime.GOARCH == "ppc64" || runtime.GOARCH == "ppc64le"

	HasAESGCMHardwareSupport = hasGCMAsmAMD64 || hasGCMAsmARM64 || hasGCMAsmS390X || hasGCMAsmPPC64
)

var ClientCipher byte

func init() {
	if HasAESGCMHardwareSupport {
		ClientCipher = 1
	}
}

type ClientInstance struct {
	sync.RWMutex
	nfsEKey *mlkem.EncapsulationKey768
	hash11  [11]byte // no more capacity
	xorKey  []byte
	minutes time.Duration
	expire  time.Time
	baseKey []byte
	ticket  []byte
}

type ClientConn struct {
	net.Conn
	instance  *ClientInstance
	baseKey   []byte
	ticket    []byte
	random    []byte
	aead      cipher.AEAD
	nonce     []byte
	peerAead  cipher.AEAD
	peerNonce []byte
	input     bytes.Reader // peerCache
}

func (i *ClientInstance) Init(nfsEKeyBytes []byte, xor uint32, minutes time.Duration) (err error) {
	if i.nfsEKey != nil {
		err = errors.New("already initialized")
		return
	}
	i.nfsEKey, err = mlkem.NewEncapsulationKey768(nfsEKeyBytes)
	if err != nil {
		return
	}
	hash256 := sha3.Sum256(nfsEKeyBytes)
	copy(i.hash11[:], hash256[:])
	if xor > 0 {
		xorKey := sha3.Sum256(nfsEKeyBytes)
		i.xorKey = xorKey[:]
	}
	i.minutes = minutes
	return
}

func (i *ClientInstance) Handshake(conn net.Conn) (net.Conn, error) {
	if i.nfsEKey == nil {
		return nil, errors.New("uninitialized")
	}
	if i.xorKey != nil {
		conn = NewXorConn(conn, i.xorKey)
	}
	c := &ClientConn{Conn: conn}

	if i.minutes > 0 {
		i.RLock()
		if time.Now().Before(i.expire) {
			c.instance = i
			c.baseKey = i.baseKey
			c.ticket = i.ticket
			i.RUnlock()
			return c, nil
		}
		i.RUnlock()
	}

	pfsDKeySeed := make([]byte, 64)
	rand.Read(pfsDKeySeed)
	pfsDKey, _ := mlkem.NewDecapsulationKey768(pfsDKeySeed)
	pfsEKeyBytes := pfsDKey.EncapsulationKey().Bytes()
	nfsKey, encapsulatedNfsKey := i.nfsEKey.Encapsulate()
	paddingLen := randBetween(100, 1000)

	clientHello := make([]byte, 5+11+1+1184+1088+5+paddingLen)
	EncodeHeader(clientHello, 1, 11+1+1184+1088)
	copy(clientHello[5:], i.hash11[:])
	clientHello[5+11] = ClientCipher
	copy(clientHello[5+11+1:], pfsEKeyBytes)
	copy(clientHello[5+11+1+1184:], encapsulatedNfsKey)
	EncodeHeader(clientHello[5+11+1+1184+1088:], 23, int(paddingLen))
	rand.Read(clientHello[5+11+1+1184+1088+5:])

	if _, err := c.Conn.Write(clientHello); err != nil {
		return nil, err
	}
	// client can send more paddings / NFS AEAD messages if needed

	_, t, l, err := ReadAndDiscardPaddings(c.Conn) // allow paddings before server hello
	if err != nil {
		return nil, err
	}

	if t != 1 {
		return nil, fmt.Errorf("unexpected type %v, expect server hello", t)
	}
	peerServerHello := make([]byte, 1088+21)
	if l != len(peerServerHello) {
		return nil, fmt.Errorf("unexpected length %v for server hello", l)
	}
	if _, err := io.ReadFull(c.Conn, peerServerHello); err != nil {
		return nil, err
	}
	encapsulatedPfsKey := peerServerHello[:1088]
	c.ticket = append(i.hash11[:], peerServerHello[1088:]...)

	pfsKey, err := pfsDKey.Decapsulate(encapsulatedPfsKey)
	if err != nil {
		return nil, err
	}
	c.baseKey = append(pfsKey, nfsKey...)

	VLESS, _ := NewAead(ClientCipher, c.baseKey, encapsulatedPfsKey, encapsulatedNfsKey).Open(nil, append(i.hash11[:], ClientCipher), c.ticket[11:], pfsEKeyBytes)
	if !bytes.Equal(VLESS, []byte("VLESS")) {
		return nil, errors.New("invalid server")
	}

	if i.minutes > 0 {
		i.Lock()
		i.expire = time.Now().Add(i.minutes)
		i.baseKey = c.baseKey
		i.ticket = c.ticket
		i.Unlock()
	}

	return c, nil
}

func (c *ClientConn) Write(b []byte) (int, error) {
	if len(b) == 0 {
		return 0, nil
	}
	var data []byte
	for n := 0; n < len(b); {
		b := b[n:]
		if len(b) > 8192 {
			b = b[:8192] // for avoiding another copy() in server's Read()
		}
		n += len(b)
		if c.aead == nil {
			data = make([]byte, 5+32+32+5+len(b)+16)
			EncodeHeader(data, 0, 32+32)
			copy(data[5:], c.ticket)
			c.random = make([]byte, 32)
			rand.Read(c.random)
			copy(data[5+32:], c.random)
			EncodeHeader(data[5+32+32:], 23, len(b)+16)
			c.aead = NewAead(ClientCipher, c.baseKey, c.random, c.ticket)
			c.nonce = make([]byte, 12)
			c.aead.Seal(data[:5+32+32+5], c.nonce, b, data[5+32+32:5+32+32+5])
		} else {
			data = make([]byte, 5+len(b)+16)
			EncodeHeader(data, 23, len(b)+16)
			c.aead.Seal(data[:5], c.nonce, b, data[:5])
			if bytes.Equal(c.nonce, MaxNonce) {
				c.aead = NewAead(ClientCipher, c.baseKey, data[5:], data[:5])
			}
		}
		IncreaseNonce(c.nonce)
		if _, err := c.Conn.Write(data); err != nil {
			return 0, err
		}
	}
	return len(b), nil
}

func (c *ClientConn) Read(b []byte) (int, error) {
	if len(b) == 0 {
		return 0, nil
	}
	if c.peerAead == nil {
		_, t, l, err := ReadAndDiscardPaddings(c.Conn) // allow paddings before random hello
		if err != nil {
			if c.instance != nil && strings.HasPrefix(err.Error(), "invalid header: ") { // 0-RTT's 0-RTT
				c.instance.Lock()
				if bytes.Equal(c.ticket, c.instance.ticket) {
					c.instance.expire = time.Now() // expired
				}
				c.instance.Unlock()
				return 0, errors.New("new handshake needed")
			}
			return 0, err
		}
		if t != 0 {
			return 0, fmt.Errorf("unexpected type %v, expect server random", t)
		}
		peerRandomHello := make([]byte, 32)
		if l != len(peerRandomHello) {
			return 0, fmt.Errorf("unexpected length %v for server random", l)
		}
		if _, err := io.ReadFull(c.Conn, peerRandomHello); err != nil {
			return 0, err
		}
		if c.random == nil {
			return 0, errors.New("empty c.random")
		}
		c.peerAead = NewAead(ClientCipher, c.baseKey, peerRandomHello, c.random)
		c.peerNonce = make([]byte, 12)
	}
	if c.input.Len() > 0 {
		return c.input.Read(b)
	}
	h, t, l, err := ReadAndDecodeHeader(c.Conn) // l: 17~17000
	if err != nil {
		return 0, err
	}
	if t != 23 {
		return 0, fmt.Errorf("unexpected type %v, expect encrypted data", t)
	}
	peerData := make([]byte, l)
	if _, err := io.ReadFull(c.Conn, peerData); err != nil {
		return 0, err
	}
	dst := peerData[:l-16]
	if len(dst) <= len(b) {
		dst = b[:len(dst)] // avoids another copy()
	}
	var peerAead cipher.AEAD
	if bytes.Equal(c.peerNonce, MaxNonce) {
		peerAead = NewAead(ClientCipher, c.baseKey, peerData, h)
	}
	_, err = c.peerAead.Open(dst[:0], c.peerNonce, peerData, h)
	if peerAead != nil {
		c.peerAead = peerAead
	}
	IncreaseNonce(c.peerNonce)
	if err != nil {
		return 0, err
	}
	if len(dst) > len(b) {
		c.input.Reset(dst[copy(b, dst):])
		dst = b // for len(dst)
	}
	return len(dst), nil
}
