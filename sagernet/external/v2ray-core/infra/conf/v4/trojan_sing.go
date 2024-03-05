package v4

import (
	"github.com/golang/protobuf/proto"
	"github.com/v2fly/v2ray-core/v5/infra/conf/cfgcommon"
	"github.com/v2fly/v2ray-core/v5/proxy/trojan_sing"
)

type TrojanSingClientConfig struct {
	Address    *cfgcommon.Address `json:"address"`
	Port       uint16             `json:"port"`
	Password   string             `json:"password"`
	ServerName string             `json:"serverName"`
	NextProtos []string           `json:"nextProtos"`
	Insecure   bool               `json:"insecure"`
}

func (v *TrojanSingClientConfig) Build() (proto.Message, error) {
	return &trojan_sing.ClientConfig{
		Address:    v.Address.Build(),
		Port:       uint32(v.Port),
		Password:   v.Password,
		ServerName: v.ServerName,
		NextProtos: v.NextProtos,
		Insecure:   v.Insecure,
	}, nil
}
