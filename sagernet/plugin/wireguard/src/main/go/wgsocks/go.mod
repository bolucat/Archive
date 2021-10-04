module wgsocks

go 1.17

require (
	github.com/Dreamacro/clash v1.6.5
	github.com/pkg/errors v0.9.1
	github.com/v2fly/v2ray-core/v4 v4.42.1
	golang.zx2c4.com/wireguard v0.0.0-20210424170727-c9db4b7aaa22
	gvisor.dev/gvisor v0.0.0
)

replace gvisor.dev/gvisor v0.0.0 => github.com/sagernet/gvisor v0.0.0-20210909160323-ce37d6df1e92

require (
	github.com/gofrs/uuid v4.0.0+incompatible // indirect
	github.com/golang/protobuf v1.5.2 // indirect
	github.com/google/btree v1.0.1 // indirect
	github.com/miekg/dns v1.1.43 // indirect
	github.com/sirupsen/logrus v1.8.1 // indirect
	golang.org/x/crypto v0.0.0-20210817164053-32db794688a5 // indirect
	golang.org/x/net v0.0.0-20210813160813-60bc85c4be6d // indirect
	golang.org/x/sys v0.0.0-20210820121016-41cdb8703e55 // indirect
	golang.org/x/time v0.0.0-20191024005414-555d28b269f0 // indirect
	google.golang.org/protobuf v1.27.1 // indirect
)
