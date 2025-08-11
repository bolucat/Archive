package encryption

import (
	"bytes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"errors"
	"io"
	"net"
	"sync"
	"time"

	"github.com/metacubex/utls/mlkem"
	"golang.org/x/crypto/hkdf"
)

type ServerSession struct {
	expire  time.Time
	cipher  byte
	baseKey []byte
	randoms sync.Map
}

type ServerInstance struct {
	sync.RWMutex
	dKeyNfs  *mlkem.DecapsulationKey768
	xor      uint32
	minutes  time.Duration
	sessions map[[21]byte]*ServerSession
	stop     bool
}

type ServerConn struct {
	net.Conn
	cipher     byte
	baseKey    []byte
	ticket     []byte
	peerRandom []byte
	peerAead   cipher.AEAD
	peerNonce  []byte
	peerCache  []byte
	aead       cipher.AEAD
	nonce      []byte
}

func (i *ServerInstance) Init(dKeyNfsData []byte, xor uint32, minutes time.Duration) (err error) {
	i.dKeyNfs, err = mlkem.NewDecapsulationKey768(dKeyNfsData)
	i.xor = xor
	if minutes > 0 {
		i.minutes = minutes
		i.sessions = make(map[[21]byte]*ServerSession)
		go func() {
			for {
				time.Sleep(time.Minute)
				now := time.Now()
				i.Lock()
				if i.stop {
					return
				}
				for index, session := range i.sessions {
					if now.After(session.expire) {
						delete(i.sessions, index)
					}
				}
				i.Unlock()
			}
		}()
	}
	return
}

func (i *ServerInstance) Close() (err error) {
	i.Lock()
	defer i.Unlock()
	i.stop = true
	return
}

func (i *ServerInstance) Handshake(conn net.Conn) (net.Conn, error) {
	if i.dKeyNfs == nil {
		return nil, errors.New("uninitialized")
	}
	if i.xor == 1 {
		conn = NewXorConn(conn, i.dKeyNfs.EncapsulationKey().Bytes())
	}
	c := &ServerConn{Conn: conn}

	peerTicketHello := make([]byte, 21+32)
	if _, err := io.ReadFull(c.Conn, peerTicketHello); err != nil {
		return nil, err
	}
	if i.minutes > 0 {
		i.RLock()
		s := i.sessions[[21]byte(peerTicketHello)]
		i.RUnlock()
		if s != nil {
			if _, replay := s.randoms.LoadOrStore([32]byte(peerTicketHello[21:]), true); !replay {
				c.cipher = s.cipher
				c.baseKey = s.baseKey
				c.ticket = peerTicketHello[:21]
				c.peerRandom = peerTicketHello[21:]
				return c, nil
			}
		}
	}

	peerHeader := make([]byte, 5)
	if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
		return nil, err
	}
	if l, _ := decodeHeader(peerHeader); l != 0 {
		c.Conn.Write(make([]byte, randBetween(100, 1000))) // make client do new handshake
		return nil, errors.New("invalid ticket")
	}

	peerClientHello := make([]byte, 1088+1184+1)
	copy(peerClientHello, peerTicketHello)
	copy(peerClientHello[53:], peerHeader)
	if _, err := io.ReadFull(c.Conn, peerClientHello[58:]); err != nil {
		return nil, err
	}
	encapsulatedNfsKey := peerClientHello[:1088]
	eKeyPfsData := peerClientHello[1088:2272]
	c.cipher = peerClientHello[2272]
	if c.cipher != 0 && c.cipher != 1 {
		return nil, errors.New("invalid cipher")
	}

	nfsKey, err := i.dKeyNfs.Decapsulate(encapsulatedNfsKey)
	if err != nil {
		return nil, err
	}
	eKeyPfs, err := mlkem.NewEncapsulationKey768(eKeyPfsData)
	if err != nil {
		return nil, err
	}
	pfsKey, encapsulatedPfsKey := eKeyPfs.Encapsulate()
	c.baseKey = append(nfsKey, pfsKey...)

	authKey := make([]byte, 32)
	hkdf.New(sha256.New, c.baseKey, encapsulatedNfsKey, eKeyPfsData).Read(authKey)
	nonce := make([]byte, 12)
	c.ticket = newAead(c.cipher, authKey).Seal(nil, nonce, []byte("VLESS"), encapsulatedPfsKey)

	padding := randBetween(100, 1000)

	serverHello := make([]byte, 1088+21+5+padding)
	copy(serverHello, encapsulatedPfsKey)
	copy(serverHello[1088:], c.ticket)
	encodeHeader(serverHello[1109:], int(padding))

	if _, err := c.Conn.Write(serverHello); err != nil {
		return nil, err
	}

	if i.minutes > 0 {
		i.Lock()
		i.sessions[[21]byte(c.ticket)] = &ServerSession{
			expire:  time.Now().Add(i.minutes),
			cipher:  c.cipher,
			baseKey: c.baseKey,
		}
		i.Unlock()
	}

	return c, nil
}

func (c *ServerConn) Read(b []byte) (int, error) {
	if len(b) == 0 {
		return 0, nil
	}
	peerHeader := make([]byte, 5)
	if c.peerAead == nil {
		if c.peerRandom == nil {
			for {
				if _, err := io.ReadFull(c.Conn, peerHeader); err != nil {
					return 0, err
				}
				peerPadding, _ := decodeHeader(peerHeader)
				if peerPadding == 0 {
					break
				}
				if _, err := io.ReadFull(c.Conn, make([]byte, peerPadding)); err != nil {
					return 0, err
				}
			}
			peerTicket := make([]byte, 21)
			copy(peerTicket, peerHeader)
			if _, err := io.ReadFull(c.Conn, peerTicket[5:]); err != nil {
				return 0, err
			}
			if !bytes.Equal(peerTicket, c.ticket) {
				return 0, errors.New("naughty boy")
			}
			c.peerRandom = make([]byte, 32)
			if _, err := io.ReadFull(c.Conn, c.peerRandom); err != nil {
				return 0, err
			}
		}
		peerKey := make([]byte, 32)
		hkdf.New(sha256.New, c.baseKey, c.peerRandom, c.ticket).Read(peerKey)
		c.peerAead = newAead(c.cipher, peerKey)
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
	peerLength, err := decodeHeader(peerHeader) // 17~17000
	if err != nil {
		return 0, err
	}
	peerData := make([]byte, peerLength)
	if _, err := io.ReadFull(c.Conn, peerData); err != nil {
		return 0, err
	}
	dst := peerData[:peerLength-16]
	if len(dst) <= len(b) {
		dst = b[:len(dst)] // max=8192 is recommended for peer
	}
	_, err = c.peerAead.Open(dst[:0], c.peerNonce, peerData, peerHeader)
	increaseNonce(c.peerNonce)
	if err != nil {
		return 0, errors.New("error")
	}
	if len(dst) > len(b) {
		c.peerCache = dst[copy(b, dst):]
		dst = b // for len(dst)
	}
	return len(dst), nil
}

func (c *ServerConn) Write(b []byte) (int, error) { // after first Read()
	if len(b) == 0 {
		return 0, nil
	}
	var data []byte
	if c.aead == nil {
		if c.peerRandom == nil {
			return 0, errors.New("can not Write() first")
		}
		data = make([]byte, 32+5+len(b)+16)
		rand.Read(data[:32])
		key := make([]byte, 32)
		hkdf.New(sha256.New, c.baseKey, data[:32], c.peerRandom).Read(key)
		c.aead = newAead(c.cipher, key)
		c.nonce = make([]byte, 12)
		encodeHeader(data[32:], len(b)+16)
		c.aead.Seal(data[:37], c.nonce, b, data[32:37])
	} else {
		data = make([]byte, 5+len(b)+16)
		encodeHeader(data, len(b)+16)
		c.aead.Seal(data[:5], c.nonce, b, data[:5])
	}
	increaseNonce(c.nonce)
	if _, err := c.Conn.Write(data); err != nil {
		return 0, err
	}
	return len(b), nil
}
