package packetaddr

import (
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
)

var _ buf.Reader = (*BufReader)(nil)

type BufReader struct {
	buf.Reader
}

func (r *BufReader) ReadMultiBuffer() (buf.MultiBuffer, error) {
	mb, err := r.Reader.ReadMultiBuffer()
	if err != nil {
		return nil, err
	}
	for index, buffer := range mb {
		extracted, addr, err := ExtractAddressFromPacket(buffer)
		if err != nil {
			return nil, newError("failed to extract address from packet").Base(err)
		}
		udpAddr := addr.(*net.UDPAddr)
		extracted.Endpoint = &net.Destination{
			Network: net.Network_UDP,
			Address: net.IPAddress(udpAddr.IP),
			Port:    net.Port(udpAddr.Port),
		}
		mb[index] = extracted
	}
	return mb, nil
}

func NewPacketReader(reader buf.Reader) *BufReader {
	return &BufReader{
		reader,
	}
}

var _ buf.Writer = (*BufWriter)(nil)

type BufWriter struct {
	buf.Writer
	dest net.Destination
}

func (w *BufWriter) WriteMultiBuffer(mb buf.MultiBuffer) error {
	for index, buffer := range mb {
		dest := &w.dest
		if buffer.Endpoint != nil {
			dest = buffer.Endpoint
		}
		packet, err := AttachAddressToPacket(buffer, &net.UDPAddr{
			IP:   dest.Address.IP(),
			Port: int(dest.Port),
		})
		if err != nil {
			return newError("failed to attach address to packet").Base(err)
		}
		mb[index] = packet
	}
	return w.Writer.WriteMultiBuffer(mb)
}

func NewPacketWriter(writer buf.Writer, dest net.Destination) *BufWriter {
	return &BufWriter{
		writer,
		dest,
	}
}
