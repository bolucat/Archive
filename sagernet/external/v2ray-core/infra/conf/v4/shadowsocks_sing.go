package v4

import (
	"github.com/golang/protobuf/proto"
	"github.com/v2fly/v2ray-core/v5/infra/conf/cfgcommon"
	"github.com/v2fly/v2ray-core/v5/proxy/shadowsocks_sing"
)

type ShadowsocksSingClientConfig struct {
	Address              *cfgcommon.Address `json:"address"`
	Port                 uint16             `json:"port"`
	Cipher               string             `json:"method"`
	Password             string             `json:"password"`
	Key                  string             `json:"key"`
	ReducedIvHeadEntropy bool               `json:"reducedIvHeadEntropy"`
}

func (v *ShadowsocksSingClientConfig) Build() (proto.Message, error) {
	return &shadowsocks_sing.ClientConfig{
		Address:              v.Address.Build(),
		Port:                 uint32(v.Port),
		Method:               v.Cipher,
		Password:             v.Password,
		Key:                  v.Key,
		ReducedIvHeadEntropy: v.ReducedIvHeadEntropy,
	}, nil
}
