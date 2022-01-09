package v4

import (
	"github.com/golang/protobuf/proto"

	"github.com/v2fly/v2ray-core/v5/common/net/pingproto"
)

type PingConfig struct {
	Protocol    string `json:"protocol"`
	Gateway4    string `json:"gateway4"`
	Gateway6    string `json:"gateway6"`
	DisableIPv6 bool   `json:"disableIPv6"`
}

func (c *PingConfig) Build() (proto.Message, error) {
	config := pingproto.Config{
		Gateway4:    c.Gateway4,
		Gateway6:    c.Gateway6,
		DisableIPv6: c.DisableIPv6,
	}
	switch c.Protocol {
	case "unprivileged":
		config.Protocol = pingproto.Protocol_Unprivileged
	case "default", "":
		config.Protocol = pingproto.Protocol_Default
	default:
		return nil, newError("unknown icmp listen protocol ", c.Protocol)
	}

	return &config, nil
}
