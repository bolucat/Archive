module libcore

go 1.17

require (
	github.com/Dreamacro/clash v1.8.0
	github.com/golang/protobuf v1.5.2
	github.com/sagernet/gomobile v0.0.0-20210905032500-701a995ff844
	github.com/sagernet/libping v0.1.1
	github.com/sagernet/sagerconnect v0.1.7
	github.com/sirupsen/logrus v1.8.1
	github.com/ulikunitz/xz v0.5.10
	github.com/v2fly/v2ray-core/v4 v4.43.0
	golang.org/x/sys v0.0.0-20211124211545-fe61309f8881
	gvisor.dev/gvisor v0.0.0
)

//replace gvisor.dev/gvisor v0.0.0 => ../gvisor
replace gvisor.dev/gvisor v0.0.0 => github.com/sagernet/gvisor v0.0.0-20211126101120-87a0de4f8727

//replace github.com/v2fly/v2ray-core/v4 v4.43.0 => ../v2ray-core
replace github.com/v2fly/v2ray-core/v4 v4.43.0 => github.com/sagernet/v2ray-core/v4 v4.41.2-0.20211126102248-43c2cb076c62

require (
	github.com/Dreamacro/go-shadowsocks2 v0.1.7 // indirect
	github.com/aead/chacha20 v0.0.0-20180709150244-8b13a72661da // indirect
	github.com/cheekybits/genny v1.0.0 // indirect
	github.com/dgryski/go-camellia v0.0.0-20191119043421-69a8a13fb23d // indirect
	github.com/dgryski/go-idea v0.0.0-20170306091226-d2fb45a411fb // indirect
	github.com/dgryski/go-metro v0.0.0-20211015221634-2661b20a2446 // indirect
	github.com/dgryski/go-rc2 v0.0.0-20150621095337-8a9021637152 // indirect
	github.com/ebfe/bcrypt_pbkdf v0.0.0-20140212075826-3c8d2dcb253a // indirect
	github.com/fsnotify/fsnotify v1.5.1 // indirect
	github.com/geeksbaek/seed v0.0.0-20180909040025-2a7f5fb92e22 // indirect
	github.com/go-task/slim-sprig v0.0.0-20210107165309-348f09dbbbc0 // indirect
	github.com/google/btree v1.0.1 // indirect
	github.com/gorilla/websocket v1.4.2 // indirect
	github.com/jhump/protoreflect v1.10.1 // indirect
	github.com/kierdavis/cfb8 v0.0.0-20180105024805-3a17c36ee2f8 // indirect
	github.com/lucas-clemente/quic-go v0.24.0 // indirect
	github.com/lunixbochs/struc v0.0.0-20200707160740-784aaebc1d40 // indirect
	github.com/marten-seemann/qtls-go1-16 v0.1.4 // indirect
	github.com/marten-seemann/qtls-go1-17 v0.1.0 // indirect
	github.com/nxadm/tail v1.4.8 // indirect
	github.com/onsi/ginkgo v1.16.5 // indirect
	github.com/pires/go-proxyproto v0.6.1 // indirect
	github.com/pkg/errors v0.9.1 // indirect
	github.com/riobard/go-bloom v0.0.0-20200614022211-cdc8013cb5b3 // indirect
	github.com/seiflotfy/cuckoofilter v0.0.0-20201222105146-bc6005554a0c // indirect
	github.com/v2fly/BrowserBridge v0.0.0-20210430233438-0570fc1d7d08 // indirect
	github.com/v2fly/VSign v0.0.0-20201108000810-e2adc24bf848 // indirect
	github.com/v2fly/ss-bloomring v0.0.0-20210312155135-28617310f63e // indirect
	github.com/xtaci/smux v1.5.16 // indirect
	go.starlark.net v0.0.0-20211013185944-b0039bd2cfe3 // indirect
	go4.org/intern v0.0.0-20211027215823-ae77deb06f29 // indirect
	go4.org/unsafe/assume-no-moving-gc v0.0.0-20211027215541-db492cf91b37 // indirect
	golang.org/x/crypto v0.0.0-20211117183948-ae814b36b871 // indirect
	golang.org/x/mod v0.5.1 // indirect
	golang.org/x/net v0.0.0-20211123203042-d83791d6bcd9 // indirect
	golang.org/x/text v0.3.7 // indirect
	golang.org/x/time v0.0.0-20211116232009-f0f3c7e86c11 // indirect
	golang.org/x/tools v0.1.7 // indirect
	golang.org/x/xerrors v0.0.0-20200804184101-5ec99f83aff1 // indirect
	golang.zx2c4.com/go118/netip v0.0.0-20211111135330-a4a02eeacf9d // indirect
	golang.zx2c4.com/wintun v0.0.0-20211104114900-415007cec224 // indirect
	golang.zx2c4.com/wireguard v0.0.0-20211123210315-387f7c461a16 // indirect
	google.golang.org/genproto v0.0.0-20211118181313-81c1377c94b1 // indirect
	google.golang.org/grpc v1.42.0 // indirect
	google.golang.org/protobuf v1.27.1 // indirect
	gopkg.in/tomb.v1 v1.0.0-20141024135613-dd632973f1e7 // indirect
	inet.af/netaddr v0.0.0-20211027220019-c74959edd3b6 // indirect
)
