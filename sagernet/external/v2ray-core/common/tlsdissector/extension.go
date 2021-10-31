package dissector

import (
	"bytes"
	"encoding/binary"
	"errors"
	"io"
)

const (
	extensionHeaderLen = 4
)

const (
	ExtServerName           uint16 = 0x00
	ExtSupportedGroups      uint16 = 0x0a
	ExtECPointFormats       uint16 = 0x0b
	ExtSignatureAlgorithms  uint16 = 0x0d
	ExtEncryptThenMac       uint16 = 0x16
	ExtExtendedMasterSecret uint16 = 0x17
	ExtSessionTicket        uint16 = 0x23
	ExtRenegotiationInfo    uint16 = 0xff01
)

var (
	ErrShortBuffer  = errors.New("short buffer")
	ErrTypeMismatch = errors.New("type mismatch")
)

type Extension interface {
	Type() uint16
	Encode() ([]byte, error)
	Decode([]byte) error
}

func NewExtension(t uint16, data []byte) (ext Extension, err error) {
	switch t {
	case ExtServerName:
		ext = new(ServerNameExtension)
	case ExtSupportedGroups:
		ext = new(SupportedGroupsExtension)
	case ExtECPointFormats:
		ext = new(ECPointFormatsExtension)
	case ExtSignatureAlgorithms:
		ext = new(SignatureAlgorithmsExtension)
	case ExtEncryptThenMac:
		ext = new(EncryptThenMacExtension)
	case ExtExtendedMasterSecret:
		ext = new(ExtendedMasterSecretExtension)
	case ExtSessionTicket:
		ext = new(SessionTicketExtension)
	case ExtRenegotiationInfo:
		ext = new(RenegotiationInfoExtension)
	default:
		ext = &unknownExtension{
			types: t,
		}
	}
	err = ext.Decode(data)
	return
}

func ReadExtension(r io.Reader) (Extension, error) {
	b := make([]byte, extensionHeaderLen)
	if _, err := io.ReadFull(r, b); err != nil {
		return nil, err
	}
	t := binary.BigEndian.Uint16(b[:2])
	bb := make([]byte, int(binary.BigEndian.Uint16(b[2:4])))
	if _, err := io.ReadFull(r, bb); err != nil {
		return nil, err
	}

	return NewExtension(t, bb)
}

func readExtensions(b []byte) (exts []Extension, err error) {
	if len(b) == 0 {
		return
	}

	br := bytes.NewReader(b)
	for br.Len() > 0 {
		var ext Extension
		ext, err = ReadExtension(br)
		if err != nil {
			return
		}
		exts = append(exts, ext)
	}
	return
}

type unknownExtension struct {
	types uint16
	raw   []byte
}

func (ext *unknownExtension) Type() uint16 {
	return ext.types
}

func (ext *unknownExtension) Encode() ([]byte, error) {
	return ext.raw, nil
}

func (ext *unknownExtension) Decode(b []byte) error {
	ext.raw = make([]byte, len(b))
	copy(ext.raw, b)
	return nil
}

type ServerNameExtension struct {
	NameType uint8
	Name     string
}

func (ext *ServerNameExtension) Type() uint16 {
	return ExtServerName
}

func (ext *ServerNameExtension) Encode() ([]byte, error) {
	buf := &bytes.Buffer{}
	binary.Write(buf, binary.BigEndian, uint16(1+2+len(ext.Name)))
	buf.WriteByte(ext.NameType)
	binary.Write(buf, binary.BigEndian, uint16(len(ext.Name)))
	buf.WriteString(ext.Name)
	return buf.Bytes(), nil
}

func (ext *ServerNameExtension) Decode(b []byte) error {
	if len(b) < 5 {
		return ErrShortBuffer
	}

	ext.NameType = b[2]
	n := int(binary.BigEndian.Uint16(b[3:]))
	if len(b[5:]) < n {
		return ErrShortBuffer
	}
	ext.Name = string(b[5 : 5+n])
	return nil
}

type SessionTicketExtension struct {
	Data []byte
}

func (ext *SessionTicketExtension) Type() uint16 {
	return ExtSessionTicket
}

func (ext *SessionTicketExtension) Encode() ([]byte, error) {
	return ext.Data, nil
}

func (ext *SessionTicketExtension) Decode(b []byte) error {
	ext.Data = make([]byte, len(b))
	copy(ext.Data, b)
	return nil
}

type ECPointFormatsExtension struct {
	Formats []uint8
}

func (ext *ECPointFormatsExtension) Type() uint16 {
	return ExtECPointFormats
}

func (ext *ECPointFormatsExtension) Encode() ([]byte, error) {
	buf := &bytes.Buffer{}
	buf.WriteByte(uint8(len(ext.Formats)))
	buf.Write(ext.Formats)
	return buf.Bytes(), nil
}

func (ext *ECPointFormatsExtension) Decode(b []byte) error {
	if len(b) < 1 {
		return ErrShortBuffer
	}

	n := int(b[0])
	if len(b[1:]) < n {
		return ErrShortBuffer
	}

	ext.Formats = make([]byte, n)
	copy(ext.Formats, b[1:])
	return nil
}

type SupportedGroupsExtension struct {
	Groups []uint16
}

func (ext *SupportedGroupsExtension) Type() uint16 {
	return ExtSupportedGroups
}

func (ext *SupportedGroupsExtension) Encode() ([]byte, error) {
	buf := &bytes.Buffer{}
	binary.Write(buf, binary.BigEndian, uint16(len(ext.Groups)*2))
	for _, group := range ext.Groups {
		binary.Write(buf, binary.BigEndian, group)
	}
	return buf.Bytes(), nil
}

func (ext *SupportedGroupsExtension) Decode(b []byte) error {
	if len(b) < 2 {
		return ErrShortBuffer
	}

	n := int(binary.BigEndian.Uint16(b)) / 2 * 2 // make it even
	if len(b[2:]) < n {
		return ErrShortBuffer
	}

	for i := 0; i < n; i += 2 {
		ext.Groups = append(ext.Groups, binary.BigEndian.Uint16(b[2+i:]))
	}
	return nil
}

type SignatureAlgorithmsExtension struct {
	Algorithms []uint16
}

func (ext *SignatureAlgorithmsExtension) Type() uint16 {
	return ExtSignatureAlgorithms
}

func (ext *SignatureAlgorithmsExtension) Encode() ([]byte, error) {
	buf := &bytes.Buffer{}
	binary.Write(buf, binary.BigEndian, uint16(len(ext.Algorithms)*2))
	for _, alg := range ext.Algorithms {
		binary.Write(buf, binary.BigEndian, alg)
	}
	return buf.Bytes(), nil
}

func (ext *SignatureAlgorithmsExtension) Decode(b []byte) error {
	if len(b) < 2 {
		return ErrShortBuffer
	}

	n := int(binary.BigEndian.Uint16(b))
	if len(b[2:]) < n {
		return ErrShortBuffer
	}

	for i := 0; i < n; i += 2 {
		ext.Algorithms = append(ext.Algorithms, binary.BigEndian.Uint16(b[2+i:]))
	}
	return nil
}

type EncryptThenMacExtension struct {
	Data []byte
}

func (ext *EncryptThenMacExtension) Type() uint16 {
	return ExtEncryptThenMac
}

func (ext *EncryptThenMacExtension) Encode() ([]byte, error) {
	return ext.Data, nil
}

func (ext *EncryptThenMacExtension) Decode(b []byte) error {
	ext.Data = make([]byte, len(b))
	copy(ext.Data, b)
	return nil
}

type ExtendedMasterSecretExtension struct {
	Data []byte
}

func (ext *ExtendedMasterSecretExtension) Type() uint16 {
	return ExtExtendedMasterSecret
}

func (ext *ExtendedMasterSecretExtension) Encode() ([]byte, error) {
	return ext.Data, nil
}

func (ext *ExtendedMasterSecretExtension) Decode(b []byte) error {
	ext.Data = make([]byte, len(b))
	copy(ext.Data, b)
	return nil
}

type RenegotiationInfoExtension struct {
	Data []byte
}

func (ext *RenegotiationInfoExtension) Type() uint16 {
	return ExtRenegotiationInfo
}

func (ext *RenegotiationInfoExtension) Encode() ([]byte, error) {
	buf := &bytes.Buffer{}
	buf.WriteByte(uint8(len(ext.Data)))
	buf.Write(ext.Data)
	return buf.Bytes(), nil
}

func (ext *RenegotiationInfoExtension) Decode(b []byte) error {
	if len(b) < 1 {
		return ErrShortBuffer
	}

	n := int(b[0])
	if len(b[1:]) < n {
		return ErrShortBuffer
	}
	ext.Data = make([]byte, n)
	copy(ext.Data, b[1:])

	return nil
}
