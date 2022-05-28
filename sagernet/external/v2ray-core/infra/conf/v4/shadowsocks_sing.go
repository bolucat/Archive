package v4

import (
	"github.com/golang/protobuf/proto"
	"github.com/v2fly/v2ray-core/v5/infra/conf/cfgcommon"
	"github.com/v2fly/v2ray-core/v5/proxy/shadowsocks_sing"
)

type ShadowsocksSingClientConfig struct {
	Address  *cfgcommon.Address `json:"address"`
	Port     uint16             `json:"port"`
	Cipher   string             `json:"method"`
	Password string             `json:"password"`
}

func (v *ShadowsocksSingClientConfig) Build() (proto.Message, error) {
	return &shadowsocks_sing.ClientConfig{
		Address:  v.Address.Build(),
		Port:     uint32(v.Port),
		Method:   v.Cipher,
		Password: v.Password,
	}, nil
}
