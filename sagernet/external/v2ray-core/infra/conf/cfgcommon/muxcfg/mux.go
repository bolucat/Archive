package muxcfg

import (
	"github.com/v2fly/v2ray-core/v5/app/proxyman"
	"github.com/v2fly/v2ray-core/v5/common/net/packetaddr"
)

type MuxConfig struct {
	Enabled        bool   `json:"enabled"`
	Concurrency    int16  `json:"concurrency"`
	PacketEncoding string `json:"packetEncoding"`
}

// Build creates MultiplexingConfig, Concurrency < 0 completely disables mux.
func (m *MuxConfig) Build() *proxyman.MultiplexingConfig {
	if m.Concurrency < 0 {
		return nil
	}

	var con uint32 = 8
	if m.Concurrency > 0 {
		con = uint32(m.Concurrency)
	}

	packetEncoding := packetaddr.PacketAddrType_None
	switch m.PacketEncoding {
	case "packet":
		packetEncoding = packetaddr.PacketAddrType_Packet
	case "xudp":
		packetEncoding = packetaddr.PacketAddrType_XUDP
	}

	return &proxyman.MultiplexingConfig{
		Enabled:        m.Enabled,
		Concurrency:    con,
		PacketEncoding: packetEncoding,
	}
}
