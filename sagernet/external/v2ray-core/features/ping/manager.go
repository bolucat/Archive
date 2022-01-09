package ping

import (
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/features"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

type Manager interface {
	features.Feature
	Dial(destination net.Destination) (internet.Connection, error)
}

func ManagerType() interface{} {
	return (*Manager)(nil)
}
