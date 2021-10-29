package shadowsocks

import (
	"github.com/v2fly/v2ray-core/v4/common"
)

var PluginCreator func(plugin string) SIP003Plugin

type SIP003Plugin interface {
	Init(localHost string, localPort string, remoteHost string, remotePort string, pluginOpts string, pluginArgs []string) error
	common.Closable
}
