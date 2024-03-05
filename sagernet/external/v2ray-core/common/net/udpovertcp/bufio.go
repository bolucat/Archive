package udpovertcp

import (
	"encoding/binary"
	"io"

	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
)

type Reader struct {
	io.Reader
}

func NewReader(reader io.Reader) *Reader {
	return &Reader{Reader: reader}
}

func NewBufferedReader(reader buf.Reader) *Reader {
	var bufferedReader *buf.BufferedReader
	if br, isBuffered := reader.(*buf.BufferedReader); isBuffered {
		bufferedReader = br
	} else {
		bufferedReader = &buf.BufferedReader{Reader: reader}
	}
	return &Reader{Reader: bufferedReader}
}

func (r *Reader) ReadMultiBuffer() (buf.MultiBuffer, error) {
	buffer := buf.New()
	addr, port, err := addrParser.ReadAddressPort(buffer, r)
	if err != nil {
		buffer.Release()
		return nil, err
	}
	endpoint := net.UDPDestination(addr, port)
	buffer.Endpoint = &endpoint
	var length uint16
	err = binary.Read(r, binary.BigEndian, &length)
	if err != nil {
		buffer.Release()
		return nil, err
	}
	buffer.Clear()
	_, err = io.ReadFull(r, buffer.Extend(int32(length)))
	if err != nil {
		buffer.Release()
		return nil, err
	}
	return buf.MultiBuffer{buffer}, nil
}

type Writer struct {
	io.Writer
	Flusher buf.Flusher
	Request *net.Destination
}

func NewWriter(writer io.Writer, request *net.Destination) *Writer {
	w := &Writer{
		Writer:  writer,
		Request: request,
	}
	if flusher, ok := writer.(buf.Flusher); ok {
		w.Flusher = flusher
	}
	return w
}

func NewBufferedWriter(writer buf.Writer, request *net.Destination) *Writer {
	var bufferedWriter *buf.BufferedWriter
	if bw, isBuffered := writer.(*buf.BufferedWriter); isBuffered {
		bufferedWriter = bw
	} else {
		bufferedWriter = buf.NewBufferedWriter(writer)
	}
	return &Writer{
		Writer:  bufferedWriter,
		Flusher: bufferedWriter,
		Request: request,
	}
}

func (w *Writer) WriteMultiBuffer(mb buf.MultiBuffer) error {
	defer buf.ReleaseMulti(mb)
	for _, packet := range mb {
		if packet.Endpoint == nil {
			packet.Endpoint = w.Request
			if w.Request == nil {
				return newError("empty packet destination")
			}
		}
		err := addrParser.WriteAddressPort(w, packet.Endpoint.Address, packet.Endpoint.Port)
		if err != nil {
			return err
		}
		err = binary.Write(w, binary.BigEndian, uint16(packet.Len()))
		if err != nil {
			return err
		}
		_, err = w.Write(packet.Bytes())
		if err != nil {
			return err
		}
	}
	if w.Flusher != nil {
		return w.Flusher.Flush()
	}
	return nil
}
