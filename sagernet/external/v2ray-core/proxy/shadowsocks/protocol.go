package shadowsocks

import (
	"bytes"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"hash/crc32"
	"io"
	"math"
	mrand "math/rand"
	gonet "net"
	"time"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/drain"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/udpovertcp"
	"github.com/v2fly/v2ray-core/v5/common/protocol"
	"github.com/v2fly/v2ray-core/v5/proxy/socks"
)

const (
	Version = 1
)

// ReadTCPSession reads a Shadowsocks TCP session from the given reader, returns its header and remaining parts.
func ReadTCPSession(user *protocol.MemoryUser, reader io.Reader, conn *ProtocolConn) (*protocol.RequestHeader, []byte, buf.Reader, error) {
	account := user.Account.(*MemoryAccount)

	var iv []byte
	var drainer drain.Drainer

	cipherFamily := account.Cipher.Family()
	if !cipherFamily.IsSpec2022() {
		hashkdf := hmac.New(sha256.New, []byte("SSBSKDF"))
		hashkdf.Write(account.Key)

		behaviorSeed := crc32.ChecksumIEEE(hashkdf.Sum(nil))

		var err error
		drainer, err = drain.NewBehaviorSeedLimitedDrainer(int64(behaviorSeed), 16+38, 3266, 64)
		if err != nil {
			return nil, nil, nil, newError("failed to initialize drainer").Base(err)
		}
	}

	buffer := buf.New()
	defer buffer.Release()

	ivLen := account.Cipher.IVSize()
	if ivLen > 0 {
		if _, err := buffer.ReadFullFrom(reader, ivLen); err != nil {
			if drainer != nil {
				drainer.AcknowledgeReceive(int(buffer.Len()))
			}
			return nil, nil, nil, drain.WithError(drainer, reader, newError("failed to read IV").Base(err))
		}

		iv = append([]byte(nil), buffer.BytesTo(ivLen)...)
	}

	r, err := account.Cipher.NewDecryptionReader(account.Key, iv, reader)
	if err != nil {
		if drainer != nil {
			drainer.AcknowledgeReceive(int(buffer.Len()))
		}
		return nil, nil, nil, drain.WithError(drainer, reader, newError("failed to initialize decoding stream").Base(err).AtError())
	}

	if conn != nil {
		conn.Reader = r
		r = conn.ProtocolReader
	}

	br := &buf.BufferedReader{Reader: r}

	request := &protocol.RequestHeader{
		Version: Version,
		User:    user,
		Command: protocol.RequestCommandTCP,
	}

	if drainer != nil {
		drainer.AcknowledgeReceive(int(buffer.Len()))
	}

	buffer.Clear()

	if cipherFamily.IsSpec2022() {
		_, err = buffer.ReadFullFrom(br, MinRequestHeaderSize)
		if err != nil {
			return nil, nil, nil, newError("failed to read response header").Base(err)
		}
		if buffer.Byte(0) != HeaderTypeClient {
			return nil, nil, nil, newError("bad request type")
		}
		epoch := int64(binary.BigEndian.Uint64(buffer.BytesRange(1, 1+8)))
		if math.Abs(float64(time.Now().Unix()-epoch)) > 30 {
			return nil, nil, nil, newError("bad timestamp")
		}
	}

	if ivError := account.CheckIV(iv); ivError != nil {
		if drainer != nil {
			drainer.AcknowledgeReceive(int(buffer.Len()))
		}
		return nil, nil, nil, drain.WithError(drainer, reader, newError("failed iv check").Base(ivError))
	}

	if request.Command != protocol.RequestCommandUDP {
		addr, port, err := socks.AddrParser.ReadAddressPort(buffer, br)
		if err != nil {
			if drainer != nil {
				drainer.AcknowledgeReceive(int(buffer.Len()))
			}
			return nil, nil, nil, drain.WithError(drainer, reader, newError("failed to read address").Base(err))
		}

		request.Address = addr
		request.Port = port
	} else {
		request.Address = net.DomainAddress(udpovertcp.UOTMagicAddress)
		request.Port = 443
	}

	if request.Address == nil {
		if drainer != nil {
			drainer.AcknowledgeReceive(int(buffer.Len()))
		}
		return nil, nil, nil, drain.WithError(drainer, reader, newError("invalid remote address."))
	}

	if cipherFamily.IsSpec2022() {
		var paddingLen uint16
		err = binary.Read(br, binary.BigEndian, &paddingLen)
		if err != nil {
			return nil, nil, nil, newError("failed to read padding length").Base(err)
		}
		if paddingLen > 0 {
			_, err = io.CopyN(io.Discard, br, int64(paddingLen))
			if err != nil {
				return nil, nil, nil, newError("failed to discard padding").Base(err)
			}
		}
	}

	return request, iv, br, nil
}

// WriteTCPRequest writes Shadowsocks request into the given writer, and returns a writer for body.
func WriteTCPRequest(request *protocol.RequestHeader, writer io.Writer, iv []byte, reader buf.Reader, conn *ProtocolConn) (buf.Writer, error) {
	user := request.User
	account := user.Account.(*MemoryAccount)
	cipherFamily := account.Cipher.Family()

	if len(iv) > 0 {
		if err := buf.WriteAllBytes(writer, iv); err != nil {
			return nil, newError("failed to write IV")
		}
	}

	w, err := account.Cipher.NewEncryptionWriter(account.Key, iv, writer)
	if err != nil {
		return nil, newError("failed to create encoding stream").Base(err).AtError()
	}

	if conn != nil {
		conn.Writer = w
		w = conn.ProtocolWriter
	}

	header := buf.New()
	if cipherFamily.IsSpec2022() {
		header.WriteByte(HeaderTypeClient)
		binary.Write(header, binary.BigEndian, uint64(time.Now().Unix()))
	}

	if err := socks.AddrParser.WriteAddressPort(header, request.Address, request.Port); err != nil {
		return nil, newError("failed to write address").Base(err)
	}

	if cipherFamily.IsSpec2022() {
		paddingLen := header.Extend(2)
		if reader != nil {
			if err = buf.CopyOnceTimeout(reader, buf.NewWriter(header), time.Millisecond*100); err != nil {
				if err == buf.ErrNotTimeoutReader || err == buf.ErrReadTimeout {
					pLen := mrand.Intn(MaxPaddingLength)
					binary.BigEndian.PutUint16(paddingLen, uint16(pLen))
					common.Must2(header.ReadFullFrom(rand.Reader, int32(pLen)))
				} else {
					return nil, newError("failed to write request payload").Base(err).AtWarning()
				}
			} else {
				binary.BigEndian.PutUint16(paddingLen, uint16(0))
			}
		}
		if err := w.WriteMultiBuffer(buf.MultiBuffer{header}); err != nil {
			return nil, newError("failed to write header").Base(err)
		}
	} else {
		if err := w.WriteMultiBuffer(buf.MultiBuffer{header}); err != nil {
			return nil, newError("failed to write header").Base(err)
		}
		if err = buf.CopyOnceTimeout(reader, w, time.Millisecond*100); err != nil && err != buf.ErrNotTimeoutReader && err != buf.ErrReadTimeout {
			return nil, newError("failed to write request payload").Base(err).AtWarning()
		}
	}

	return w, nil
}

func ReadTCPResponse(user *protocol.MemoryUser, command protocol.RequestCommand, reader io.Reader, requestIv []byte, conn *ProtocolConn) (buf.Reader, error) {
	account := user.Account.(*MemoryAccount)
	cipherFamily := account.Cipher.Family()
	var iv []byte
	var drainer drain.Drainer

	if !cipherFamily.IsSpec2022() {

		hashkdf := hmac.New(sha256.New, []byte("SSBSKDF"))
		hashkdf.Write(account.Key)

		behaviorSeed := crc32.ChecksumIEEE(hashkdf.Sum(nil))

		var err error
		drainer, err = drain.NewBehaviorSeedLimitedDrainer(int64(behaviorSeed), 16+38, 3266, 64)
		if err != nil {
			return nil, newError("failed to initialize drainer").Base(err)
		}

	}

	if account.Cipher.IVSize() > 0 {
		iv = make([]byte, account.Cipher.IVSize())
		if n, err := io.ReadFull(reader, iv); err != nil {
			return nil, newError("failed to read IV").Base(err)
		} else if drainer != nil { // nolint: revive
			drainer.AcknowledgeReceive(n)
		}
	}

	if ivError := account.CheckIV(iv); ivError != nil {
		return nil, drain.WithError(drainer, reader, newError("failed iv check").Base(ivError))
	}

	r, err := account.Cipher.NewDecryptionReader(account.Key, iv, reader)

	if conn != nil {
		conn.Reader = r
		r = conn.ProtocolReader
	}

	if cipherFamily.IsSpec2022() {

		header := buf.StackNew()
		defer header.Release()

		br := &buf.BufferedReader{Reader: r}
		_, err = header.ReadFullFrom(br, MinResponseHeaderSize)
		if err != nil {
			return nil, err
		}
		responseType := header.Byte(0)
		if responseType != HeaderTypeServer {
			return nil, newError("bad response type")
		}
		epoch := int64(binary.BigEndian.Uint64(header.BytesRange(1, 1+8)))
		if math.Abs(float64(time.Now().Unix()-epoch)) > 30 {
			return nil, newError("bad timestamp")
		}
		if bytes.Compare(requestIv, header.BytesFrom(1+8)) != 0 {
			return nil, newError("bad request iv in response")
		}
		r = br
	}

	return r, err
}

func WriteTCPResponse(request *protocol.RequestHeader, writer io.Writer, requestIV []byte, iv []byte, conn *ProtocolConn) (buf.Writer, error) {
	user := request.User
	account := user.Account.(*MemoryAccount)
	cipherFamily := account.Cipher.Family()

	if len(iv) > 0 {
		if err := buf.WriteAllBytes(writer, iv); err != nil {
			return nil, newError("failed to write IV.").Base(err)
		}
	}

	w, err := account.Cipher.NewEncryptionWriter(account.Key, iv, writer)

	if err == nil && conn != nil {
		conn.Writer = w
		w = conn.ProtocolWriter
	}

	if cipherFamily.IsSpec2022() {
		bw := buf.NewBufferedWriter(w)
		bw.WriteByte(HeaderTypeServer)
		binary.Write(bw, binary.BigEndian, uint64(time.Now().Unix()))
		bw.Write(requestIV)
		bw.SetBuffered(false)
	}

	return w, err
}

func EncodeUDPPacket(request *protocol.RequestHeader, payload []byte, session *udpSession, plugin ProtocolPlugin) (*buf.Buffer, error) {
	user := request.User
	account := user.Account.(*MemoryAccount)
	cipherFamily := account.Cipher.Family()

	buffer := buf.New()
	var ivLen int32

	switch cipherFamily {
	case CipherFamilyAEADSpec2022:
		ivLen = 24
	case CipherFamilyAEADSpec2022UDPBlock:
		ivLen = 0
	default:
		ivLen = account.Cipher.IVSize()
	}
	if ivLen > 0 {
		common.Must2(buffer.ReadFullFrom(rand.Reader, ivLen))
	}

	if cipherFamily.IsSpec2022() {

		binary.Write(buffer, binary.BigEndian, session.sessionId)
		binary.Write(buffer, binary.BigEndian, session.nextPacketId())
		buffer.WriteByte(session.headerType)
		binary.Write(buffer, binary.BigEndian, uint64(time.Now().Unix()))
		if session.headerType == HeaderTypeServer {
			binary.Write(buffer, binary.BigEndian, session.remoteSessionId)
		}
		binary.Write(buffer, binary.BigEndian, uint16(0)) // padding length
	}

	if err := socks.AddrParser.WriteAddressPort(buffer, request.Address, request.Port); err != nil {
		buffer.Release()
		return nil, newError("failed to write address").Base(err)
	}

	buffer.Write(payload)

	if plugin != nil {
		if newBuffer, err := plugin.EncodePacket(buffer, ivLen); err == nil {
			buffer = newBuffer
		} else {
			return nil, newError("failed to encode UDP payload").Base(err)
		}
	}

	if err := account.Cipher.EncodePacket(account.Key, buffer); err != nil {
		buffer.Release()
		return nil, newError("failed to encrypt UDP payload").Base(err)
	}

	return buffer, nil
}

func DecodeUDPPacket(user *protocol.MemoryUser, payload *buf.Buffer, session *udpSession, plugin ProtocolPlugin) (*protocol.RequestHeader, *buf.Buffer, error) {
	account := user.Account.(*MemoryAccount)
	cipherFamily := account.Cipher.Family()

	var ivLen int32
	switch cipherFamily {
	case CipherFamilyAEADSpec2022:
		ivLen = 24
	case CipherFamilyAEADSpec2022UDPBlock:
		ivLen = 0
	default:
		ivLen = account.Cipher.IVSize()
	}
	var iv []byte
	if ivLen > 0 {
		iv = make([]byte, ivLen)
		copy(iv, payload.BytesTo(ivLen))
	}

	if err := account.Cipher.DecodePacket(account.Key, payload); err != nil {
		return nil, nil, newError("failed to decrypt UDP payload").Base(err)
	}

	if plugin != nil {
		if newBuffer, err := plugin.DecodePacket(payload); err == nil {
			payload = newBuffer
		} else {
			return nil, nil, newError("failed to decode UDP payload").Base(err)
		}
	}

	request := &protocol.RequestHeader{
		Version: Version,
		User:    user,
		Command: protocol.RequestCommandUDP,
	}

	if cipherFamily.IsSpec2022() {
		// packetHeader
		var sessionId uint64
		err := binary.Read(payload, binary.BigEndian, &sessionId)
		if err != nil {
			return nil, nil, err
		}

		if session.remoteSessionId == 0 {
			session.remoteSessionId = sessionId
		} else if sessionId != session.remoteSessionId {
			session.lastRemoteSessionId = session.remoteSessionId
			session.remoteSessionId = sessionId
		}

		var packetId uint64
		err = binary.Read(payload, binary.BigEndian, &packetId)
		if err != nil {
			return nil, nil, err
		}

		headerType, err := payload.ReadBytes(1)
		if err != nil {
			return nil, nil, err
		}
		if headerType[0] == session.headerType {
			return nil, nil, newError("bad header type")
		}

		var epoch uint64
		err = binary.Read(payload, binary.BigEndian, &epoch)
		if err != nil {
			return nil, nil, err
		}
		if math.Abs(float64(uint64(time.Now().Unix())-epoch)) > 30 {
			return nil, nil, newError("bad timestamp")
		}
		if session.headerType == HeaderTypeClient {
			var clientSessionId uint64
			err = binary.Read(payload, binary.BigEndian, &clientSessionId)
			if err != nil {
				return nil, nil, err
			}

			if clientSessionId != session.sessionId {
				return nil, nil, newError("bad session id")
			}
		}
		var paddingLength uint16
		err = binary.Read(payload, binary.BigEndian, &paddingLength)
		if err != nil {
			return nil, nil, newError("failed to read padding length").Base(err)
		}
		_, err = payload.ReadBytes(int32(paddingLength))
		if err != nil {
			return nil, nil, newError("failed to discard padding")
		}
	}

	payload.SetByte(0, payload.Byte(0)&0x0F)

	addr, port, err := socks.AddrParser.ReadAddressPort(nil, payload)
	if err != nil {
		return nil, nil, newError("failed to parse address").Base(err)
	}

	request.Address = addr
	request.Port = port

	return request, payload, nil
}

type UDPReader struct {
	Reader  io.Reader
	User    *protocol.MemoryUser
	Plugin  ProtocolPlugin
	session *udpSession
}

func (v *UDPReader) ReadMultiBuffer() (buf.MultiBuffer, error) {
	buffer := buf.New()
	_, err := buffer.ReadFrom(v.Reader)
	if err != nil {
		buffer.Release()
		return nil, err
	}
	header, payload, err := DecodeUDPPacket(v.User, buffer, v.session, v.Plugin)
	if err != nil {
		buffer.Release()
		return nil, err
	}
	endpoint := header.Destination()
	payload.Endpoint = &endpoint
	return buf.MultiBuffer{payload}, nil
}

func (v *UDPReader) ReadFrom(p []byte) (n int, addr gonet.Addr, err error) {
	buffer := buf.New()
	_, err = buffer.ReadFrom(v.Reader)
	if err != nil {
		buffer.Release()
		return 0, nil, err
	}
	vaddr, payload, err := DecodeUDPPacket(v.User, buffer, v.session, v.Plugin)
	if err != nil {
		buffer.Release()
		return 0, nil, err
	}
	n = copy(p, payload.Bytes())
	payload.Release()
	return n, &gonet.UDPAddr{IP: vaddr.Address.IP(), Port: int(vaddr.Port)}, nil
}

type UDPWriter struct {
	Writer  io.Writer
	Request *protocol.RequestHeader
	Plugin  ProtocolPlugin
	session *udpSession
}

func (w *UDPWriter) WriteMultiBuffer(mb buf.MultiBuffer) error {
	for _, buffer := range mb {
		if buffer == nil {
			continue
		}
		request := w.Request
		if buffer.Endpoint != nil {
			request = &protocol.RequestHeader{
				User:    w.Request.User,
				Address: buffer.Endpoint.Address,
				Port:    buffer.Endpoint.Port,
			}
		}
		packet, err := EncodeUDPPacket(request, buffer.Bytes(), w.session, w.Plugin)
		buffer.Release()
		if err != nil {
			buf.ReleaseMulti(mb)
			return err
		}
		_, err = w.Writer.Write(packet.Bytes())
		packet.Release()
		if err != nil {
			buf.ReleaseMulti(mb)
			return err
		}
	}
	return nil
}

// Write implements io.Writer.
func (w *UDPWriter) Write(payload []byte) (int, error) {
	packet, err := EncodeUDPPacket(w.Request, payload, w.session, w.Plugin)
	if err != nil {
		return 0, err
	}
	_, err = w.Writer.Write(packet.Bytes())
	packet.Release()
	return len(payload), err
}

func (w *UDPWriter) WriteTo(payload []byte, addr gonet.Addr) (n int, err error) {
	request := *w.Request
	udpAddr := addr.(*gonet.UDPAddr)
	request.Command = protocol.RequestCommandUDP
	request.Address = net.IPAddress(udpAddr.IP)
	request.Port = net.Port(udpAddr.Port)
	packet, err := EncodeUDPPacket(&request, payload, w.session, w.Plugin)
	if err != nil {
		return 0, err
	}
	_, err = w.Writer.Write(packet.Bytes())
	packet.Release()
	return len(payload), err
}

func remapToPrintable(input []byte) {
	const charSet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!#$%&()*+,./:;<=>?@[]^_`{|}~\\\""
	seed := mrand.New(mrand.NewSource(int64(crc32.ChecksumIEEE(input))))
	for i := range input {
		input[i] = charSet[seed.Intn(len(charSet))]
	}
}
