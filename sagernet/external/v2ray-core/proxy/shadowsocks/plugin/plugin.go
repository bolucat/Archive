package plugin

import (
	"github.com/v2fly/v2ray-core/v4/proxy/shadowsocks"
	"github.com/v2fly/v2ray-core/v4/proxy/shadowsocks/plugin/external"
	"github.com/v2fly/v2ray-core/v4/proxy/shadowsocks/plugin/self"
)

func init() {
	shadowsocks.PluginCreator = func(plugin string) shadowsocks.SIP003Plugin {
		if plugin == "v2ray" || plugin == "v2ray-plugin" {
			return &self.Plugin{}
		}

		return &external.Plugin{Plugin: plugin}
	}
}
