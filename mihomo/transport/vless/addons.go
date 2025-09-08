package vless

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
)

func ReadAddons(data []byte) (*Addons, error) {
	reader := bytes.NewReader(data)
	var addons Addons
	for reader.Len() > 0 {
		protoHeader, err := reader.ReadByte()
		if err != nil {
			return nil, err
		}
		switch protoHeader {
		case (1 << 3) | 2:
			flowLen, err := binary.ReadUvarint(reader)
			if err != nil {
				return nil, err
			}
			flowBytes := make([]byte, flowLen)
			_, err = io.ReadFull(reader, flowBytes)
			if err != nil {
				return nil, err
			}
			addons.Flow = string(flowBytes)
		case (2 << 3) | 2:
			seedLen, err := binary.ReadUvarint(reader)
			if err != nil {
				return nil, err
			}
			seedBytes := make([]byte, seedLen)
			_, err = io.ReadFull(reader, seedBytes)
			if err != nil {
				return nil, err
			}
			addons.Seed = seedBytes
		default:
			return nil, fmt.Errorf("unknown protobuf message header: %v", protoHeader)
		}
	}
	return &addons, nil
}

func WriteAddons(addons *Addons) []byte {
	var writer bytes.Buffer
	if len(addons.Flow) > 0 {
		writer.WriteByte((1 << 3) | 2)
		writer.Write(binary.AppendUvarint(nil, uint64(len(addons.Flow))))
		writer.WriteString(addons.Flow)
	}
	if len(addons.Seed) > 0 {
		writer.WriteByte((2 << 3) | 2)
		writer.Write(binary.AppendUvarint(nil, uint64(len(addons.Seed))))
		writer.Write(addons.Seed)
	}
	return writer.Bytes()
}
