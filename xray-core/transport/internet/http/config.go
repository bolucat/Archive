package http

import (
	"github.com/xtls/xray-core/common"
	"github.com/xtls/xray-core/common/dice"
	"github.com/xtls/xray-core/transport/internet"
)

func (c *Config) getHosts() []string {
	if len(c.Host) == 0 {
		return []string{"www.example.com"}
	}
	return c.Host
}

func (c *Config) isValidHost(host string) bool {
	if len(c.Host) == 0 {
		return true
	}
	hosts := c.getHosts()
	for _, h := range hosts {
		if internet.IsValidHTTPHost(host, h) {
			return true
		}
	}
	return false
}

func (c *Config) getRandomHost() string {
	hosts := c.getHosts()
	return hosts[dice.Roll(len(hosts))]
}

func (c *Config) getNormalizedPath() string {
	if c.Path == "" {
		return "/"
	}
	if c.Path[0] != '/' {
		return "/" + c.Path
	}
	return c.Path
}

func init() {
	common.Must(internet.RegisterProtocolConfigCreator(protocolName, func() interface{} {
		return new(Config)
	}))
}
