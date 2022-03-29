package shadowsocks

import (
	"crypto/aes"
	"crypto/cipher"
	"io"
	"math/rand"
	"sync/atomic"

	"lukechampine.com/blake3"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/crypto"
	"github.com/v2fly/v2ray-core/v5/common/protocol"
)

const (
	HeaderTypeClient       = 0
	HeaderTypeServer       = 1
	HeaderTypeClientPacket = 2
	HeaderTypeServerPacket = 3
	MaxPaddingLength       = 900
	SaltSize               = 32
	PacketNonceSize        = 24
	MinRequestHeaderSize   = 1 + 8
	MinResponseHeaderSize  = MinRequestHeaderSize + SaltSize
)

var _ Cipher = (*AEAD2022Cipher)(nil)

type AEAD2022Cipher struct {
	KeyBytes           int32
	AEADAuthCreator    func(key []byte) cipher.AEAD
	UDPBlockCreator    func(key []byte) cipher.Block
	UDPAEADAuthCreator func(key []byte) cipher.AEAD
}

func (c *AEAD2022Cipher) Family() CipherFamily {
	if c.UDPBlockCreator != nil {
		return CipherFamilyAEADSpec2022UDPBlock
	} else {
		return CipherFamilyAEADSpec2022
	}
}

func (c *AEAD2022Cipher) KeySize() int32 {
	return c.KeyBytes
}

func (c *AEAD2022Cipher) IVSize() int32 {
	return SaltSize
}

func (c *AEAD2022Cipher) tcpAuthenticator(key []byte, iv []byte) *crypto.AEADAuthenticator {
	subkey := make([]byte, c.KeyBytes)
	deriveKey(key, iv, subkey)
	aead := c.AEADAuthCreator(subkey)
	nonce := crypto.GenerateAEADNonceWithSize(aead.NonceSize())
	return &crypto.AEADAuthenticator{
		AEAD:           aead,
		NonceGenerator: nonce,
	}
}

func (c *AEAD2022Cipher) NewEncryptionWriter(key []byte, iv []byte, writer io.Writer) (buf.Writer, error) {
	auth := c.tcpAuthenticator(key, iv)
	return crypto.NewAuthenticationWriter(auth, &crypto.AEADChunkSizeParser{
		Auth: auth,
	}, writer, protocol.TransferTypeStream, nil), nil
}

func (c *AEAD2022Cipher) NewDecryptionReader(key []byte, iv []byte, reader io.Reader) (buf.Reader, error) {
	auth := c.tcpAuthenticator(key, iv)
	return crypto.NewAuthenticationReader(auth, &crypto.AEADChunkSizeParser{
		Auth: auth,
	}, reader, protocol.TransferTypeStream, nil), nil
}

func (c *AEAD2022Cipher) EncodePacket(key []byte, b *buf.Buffer) error {
	payloadLen := b.Len()
	if c.UDPBlockCreator != nil {
		// aes
		packetHeader := b.BytesTo(aes.BlockSize)
		subKey := make([]byte, c.KeyBytes)
		deriveKey(key, packetHeader[:8], subKey)

		auth := &crypto.AEADAuthenticator{
			AEAD:           c.AEADAuthCreator(subKey),
			NonceGenerator: crypto.GenerateStaticBytes(packetHeader[4:16]),
		}

		b.Extend(int32(auth.Overhead()))
		_, err := auth.Seal(b.BytesTo(aes.BlockSize), b.BytesRange(aes.BlockSize, payloadLen))
		c.UDPBlockCreator(key).Encrypt(packetHeader, packetHeader)
		return err
	} else {
		// xchacha
		auth := &crypto.AEADAuthenticator{
			AEAD:           c.UDPAEADAuthCreator(key),
			NonceGenerator: crypto.GenerateStaticBytes(b.BytesTo(PacketNonceSize)),
		}
		b.Extend(int32(auth.Overhead()))
		_, err := auth.Seal(b.BytesTo(PacketNonceSize), b.BytesRange(PacketNonceSize, payloadLen))
		return err
	}
}

func (c *AEAD2022Cipher) DecodePacket(key []byte, b *buf.Buffer) error {
	var nonceIndex int32
	var nonceLen int32
	payloadLen := b.Len()
	var auth *crypto.AEADAuthenticator
	if c.UDPBlockCreator != nil {
		if b.Len() <= aes.BlockSize {
			return newError("insufficient data: ", b.Len())
		}
		packetHeader := b.BytesTo(aes.BlockSize)
		c.UDPBlockCreator(key).Decrypt(packetHeader, packetHeader)
		subKey := make([]byte, c.KeyBytes)
		deriveKey(key, packetHeader[:8], subKey)
		auth = &crypto.AEADAuthenticator{
			AEAD:           c.AEADAuthCreator(subKey),
			NonceGenerator: crypto.GenerateStaticBytes(packetHeader[4:16]),
		}
		nonceIndex = 0
		nonceLen = 16
	} else {
		auth = &crypto.AEADAuthenticator{
			AEAD:           c.UDPAEADAuthCreator(key),
			NonceGenerator: crypto.GenerateStaticBytes(b.BytesTo(PacketNonceSize)),
		}
		nonceIndex = PacketNonceSize
		nonceLen = PacketNonceSize
	}
	bbb, err := auth.Open(b.BytesTo(nonceLen), b.BytesRange(nonceLen, payloadLen))
	if err != nil {
		return err
	}
	b.Resize(nonceIndex, int32(len(bbb)))
	return nil
}

func deriveKey(secret, salt, outKey []byte) {
	sessionKey := make([]byte, len(secret)+len(salt))
	copy(sessionKey, secret)
	copy(sessionKey[len(secret):], salt)
	blake3.DeriveKey(outKey, "shadowsocks 2022 session subkey", sessionKey)
}

type udpSession struct {
	sessionId           uint64
	packetId            uint64
	headerType          byte
	remoteSessionId     uint64
	lastRemoteSessionId uint64
}

func (s *udpSession) nextPacketId() uint64 {
	return atomic.AddUint64(&s.packetId, 1)
}

func newUDPSession(server bool) *udpSession {
	s := new(udpSession)
	s.sessionId = rand.Uint64()
	if server {
		s.headerType = HeaderTypeServer
		s.packetId = 1<<63 - 1
	}
	return s
}