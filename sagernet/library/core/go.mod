module libcore

go 1.18

require (
	github.com/Dreamacro/clash v1.10.6
	github.com/golang/protobuf v1.5.2
	github.com/pion/stun v0.3.6-0.20211201014640-159901e761c9
	github.com/sagernet/gomobile v0.0.0-20220214172500-89df302623c8
	github.com/sagernet/libping v0.1.1
	github.com/sagernet/sagerconnect v0.1.7
	github.com/sirupsen/logrus v1.8.1
	github.com/ulikunitz/xz v0.5.10
	github.com/v2fly/v2ray-core/v5 v5.0.6
	golang.org/x/net v0.0.0-20220513224357-95641704303c
	golang.org/x/sys v0.0.0-20220513210249-45d2b4557a2a
	gvisor.dev/gvisor v0.0.0
)

// https://github.com/google/gvisor/releases/tag/release-20211129.0
//replace gvisor.dev/gvisor => ../gvisor
replace gvisor.dev/gvisor => github.com/sagernet/gvisor v0.0.0-20220402114650-763d12dc953e

//replace github.com/v2fly/v2ray-core/v5 => ../v2ray-core

replace github.com/v2fly/v2ray-core/v5 => github.com/sagernet/v2ray-core/v5 v5.0.11-0.20220515030048-d8f46c81b373

//replace github.com/sagernet/sing => ../sing

require (
	github.com/Dreamacro/go-shadowsocks2 v0.1.8 // indirect
	github.com/aead/chacha20 v0.0.0-20180709150244-8b13a72661da // indirect
	github.com/cheekybits/genny v1.0.0 // indirect
	github.com/dgryski/go-camellia v0.0.0-20191119043421-69a8a13fb23d // indirect
	github.com/dgryski/go-idea v0.0.0-20170306091226-d2fb45a411fb // indirect
	github.com/dgryski/go-metro v0.0.0-20211217172704-adc40b04c140 // indirect
	github.com/dgryski/go-rc2 v0.0.0-20150621095337-8a9021637152 // indirect
	github.com/fsnotify/fsnotify v1.5.1 // indirect
	github.com/geeksbaek/seed v0.0.0-20180909040025-2a7f5fb92e22 // indirect
	github.com/go-task/slim-sprig v0.0.0-20210107165309-348f09dbbbc0 // indirect
	github.com/google/btree v1.0.1 // indirect
	github.com/gorilla/websocket v1.5.0 // indirect
	github.com/jhump/protoreflect v1.12.0 // indirect
	github.com/kierdavis/cfb8 v0.0.0-20180105024805-3a17c36ee2f8 // indirect
	github.com/klauspost/cpuid/v2 v2.0.12 // indirect
	github.com/lucas-clemente/quic-go v0.27.0 // indirect
	github.com/lunixbochs/struc v0.0.0-20200707160740-784aaebc1d40 // indirect
	github.com/marten-seemann/qtls-go1-16 v0.1.5 // indirect
	github.com/marten-seemann/qtls-go1-17 v0.1.1 // indirect
	github.com/marten-seemann/qtls-go1-18 v0.1.1 // indirect
	github.com/nxadm/tail v1.4.8 // indirect
	github.com/onsi/ginkgo v1.16.5 // indirect
	github.com/pires/go-proxyproto v0.6.2 // indirect
	github.com/pkg/errors v0.9.1 // indirect
	github.com/riobard/go-bloom v0.0.0-20200614022211-cdc8013cb5b3 // indirect
	github.com/sagernet/sing v0.0.0-20220515005650-1cbc30ebc049 // indirect
	github.com/seiflotfy/cuckoofilter v0.0.0-20220411075957-e3b120b3f5fb // indirect
	github.com/v2fly/BrowserBridge v0.0.0-20210430233438-0570fc1d7d08 // indirect
	github.com/v2fly/ss-bloomring v0.0.0-20210312155135-28617310f63e // indirect
	github.com/xtaci/smux v1.5.16 // indirect
	github.com/xtls/go v0.0.0-20210920065950-d4af136d3672 // indirect
	go.starlark.net v0.0.0-20220328144851-d1966c6b9fcd // indirect
	go4.org/intern v0.0.0-20220301175310-a089fc204883 // indirect
	go4.org/unsafe/assume-no-moving-gc v0.0.0-20211027215541-db492cf91b37 // indirect
	golang.org/x/crypto v0.0.0-20220513210258-46612604a0f9 // indirect
	golang.org/x/mod v0.6.0-dev.0.20220106191415-9b9b3d81d5e3 // indirect
	golang.org/x/text v0.3.7 // indirect
	golang.org/x/time v0.0.0-20220411224347-583f2d630306 // indirect
	golang.org/x/tools v0.1.11-0.20220325154526-54af36eca237 // indirect
	golang.org/x/xerrors v0.0.0-20200804184101-5ec99f83aff1 // indirect
	golang.zx2c4.com/wintun v0.0.0-20211104114900-415007cec224 // indirect
	golang.zx2c4.com/wireguard v0.0.0-20220407013110-ef5c587f782d // indirect
	google.golang.org/genproto v0.0.0-20220324131243-acbaeb5b85eb // indirect
	google.golang.org/grpc v1.46.0 // indirect
	google.golang.org/protobuf v1.28.0 // indirect
	gopkg.in/tomb.v1 v1.0.0-20141024135613-dd632973f1e7 // indirect
	inet.af/netaddr v0.0.0-20211027220019-c74959edd3b6 // indirect
	lukechampine.com/blake3 v1.1.7 // indirect
)
