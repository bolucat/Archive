package encryption

import (
	"bytes"
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"io"
	"net"
	"runtime"
	"sync"
	"time"

	"github.com/metacubex/utls/mlkem"
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
	nfsEKey      *mlkem.EncapsulationKey768
	nfsEKeyBytes []byte
	xor          uint32
	minutes      time.Duration
	expire       time.Time
	baseKey      []byte
	ticket       []byte
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
	peerCache []byte
}

func (i *ClientInstance) Init(nfsEKeyBytes []byte, xor uint32, minutes time.Duration) (err error) {
	i.nfsEKey, err = mlkem.NewEncapsulationKey768(nfsEKeyBytes)
	if xor > 0 {
		i.nfsEKeyBytes = nfsEKeyBytes
		i.xor = xor
	}
	i.minutes = minutes
	return
}

func (i *ClientInstance) Handshake(conn net.Conn) (net.Conn, error) {
	if i.nfsEKey == nil {
		return nil, errors.New("uninitialized")
	}
	if i.xor > 0 {
		conn = NewXorConn(conn, i.nfsEKeyBytes)
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

	clientHello := make([]byte, 1+1184+1088+5+paddingLen)
	clientHello[0] = ClientCipher
	copy(clientHello[1:], pfsEKeyBytes)
	copy(clientHello[1185:], encapsulatedNfsKey)
	EncodeHeader(clientHello[2273:], int(paddingLen))
	rand.Read(clientHello[2278:])

	if _, err := c.Conn.Write(clientHello); err != nil {
		return nil, err
	}
	// we can send more padding if needed

	peerServerHello := make([]byte, 1088+21)
	if _, err := io.ReadFull(c.Conn, peerServerHello); err != nil {
		return nil, err
	}
	encapsulatedPfsKey := peerServerHello[:1088]
	c.ticket = peerServerHello[1088:]

	pfsKey, err := pfsDKey.Decapsulate(encapsulatedPfsKey)
	if err != nil {
		return nil, err
	}
	c.baseKey = append(pfsKey, nfsKey...)

	nonce := [12]byte{ClientCipher}
	VLESS, _ := NewAead(ClientCipher, c.baseKey, encapsulatedPfsKey, encapsulatedNfsKey).Open(nil, nonce[:], c.ticket, pfsEKeyBytes)
	if !bytes.Equal(VLESS, []byte("VLESS")) { // TODO: more messages
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
			c.random = make([]byte, 32)
			rand.Read(c.random)
			c.aead = NewAead(ClientCipher, c.baseKey, c.random, c.ticket)
			c.nonce = make([]byte, 12)
			data = make([]byte, 21+32+5+len(b)+16)
			copy(data, c.ticket)
			copy(data[21:], c.random)
			EncodeHeader(data[53:], len(b)+16)
			c.aead.Seal(data[:58], c.nonce, b, data[53:58])
		} else {
			data = make([]byte, 5+len(b)+16)
			EncodeHeader(data, len(b)+16)
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
	peerHeader := make([]byte, 5)
	if c.peerAead == nil {
		if c.instance == nil {
			for {
				if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
					return 0, err
				}
				peerPaddingLen, _ := DecodeHeader(peerHeader)
				if peerPaddingLen == 0 {
					break
				}
				if _, err := io.ReadFull(c.Conn, make([]byte, peerPaddingLen)); err != nil {
					return 0, err
				}
			}
		} else {
			if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
				return 0, err
			}
		}
		peerRandom := make([]byte, 32)
		copy(peerRandom, peerHeader)
		if _, err := io.ReadFull(c.Conn, peerRandom[5:]); err != nil {
			return 0, err
		}
		if c.random == nil {
			return 0, errors.New("empty c.random")
		}
		c.peerAead = NewAead(ClientCipher, c.baseKey, peerRandom, c.random)
		c.peerNonce = make([]byte, 12)
	}
	if len(c.peerCache) != 0 {
		n := copy(b, c.peerCache)
		c.peerCache = c.peerCache[n:]
		return n, nil
	}
	if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
		return 0, err
	}
	peerLength, err := DecodeHeader(peerHeader) // 17~17000
	if err != nil {
		if c.instance != nil {
			c.instance.Lock()
			if bytes.Equal(c.ticket, c.instance.ticket) {
				c.instance.expire = time.Now() // expired
			}
			c.instance.Unlock()
		}
		return 0, err
	}
	peerData := make([]byte, peerLength)
	if _, err := io.ReadFull(c.Conn, peerData); err != nil {
		return 0, err
	}
	dst := peerData[:peerLength-16]
	if len(dst) <= len(b) {
		dst = b[:len(dst)] // avoids another copy()
	}
	var peerAead cipher.AEAD
	if bytes.Equal(c.peerNonce, MaxNonce) {
		peerAead = NewAead(ClientCipher, c.baseKey, peerData, peerHeader)
	}
	_, err = c.peerAead.Open(dst[:0], c.peerNonce, peerData, peerHeader)
	if peerAead != nil {
		c.peerAead = peerAead
	}
	IncreaseNonce(c.peerNonce)
	if err != nil {
		return 0, err
	}
	if len(dst) > len(b) {
		c.peerCache = dst[copy(b, dst):]
		dst = b // for len(dst)
	}
	return len(dst), nil
}
