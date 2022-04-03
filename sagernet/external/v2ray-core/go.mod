module github.com/v2fly/v2ray-core/v5

go 1.18

require (
	github.com/aead/chacha20 v0.0.0-20180709150244-8b13a72661da
	github.com/dgryski/go-camellia v0.0.0-20191119043421-69a8a13fb23d
	github.com/dgryski/go-idea v0.0.0-20170306091226-d2fb45a411fb
	github.com/dgryski/go-rc2 v0.0.0-20150621095337-8a9021637152
	github.com/geeksbaek/seed v0.0.0-20180909040025-2a7f5fb92e22
	github.com/go-chi/chi/v5 v5.0.7
	github.com/go-chi/render v1.0.1
	github.com/go-playground/validator/v10 v10.10.1
	github.com/golang/mock v1.6.0
	github.com/golang/protobuf v1.5.2
	github.com/google/go-cmp v0.5.7
	github.com/gorilla/websocket v1.5.0
	github.com/jhump/protoreflect v1.12.0
	github.com/kierdavis/cfb8 v0.0.0-20180105024805-3a17c36ee2f8
	github.com/lucas-clemente/quic-go v0.26.0
	github.com/marten-seemann/qtls-go1-17 v0.1.1
	github.com/miekg/dns v1.1.47
	github.com/pires/go-proxyproto v0.6.2
	github.com/seiflotfy/cuckoofilter v0.0.0-20220312154859-af7fbb8e765b
	github.com/stretchr/testify v1.7.1
	github.com/v2fly/BrowserBridge v0.0.0-20210430233438-0570fc1d7d08
	github.com/v2fly/VSign v0.0.0-20201108000810-e2adc24bf848
	github.com/v2fly/ss-bloomring v0.0.0-20210312155135-28617310f63e
	github.com/xtls/go v0.0.0-20210920065950-d4af136d3672
	go.starlark.net v0.0.0-20220328144851-d1966c6b9fcd
	golang.org/x/crypto v0.0.0-20220321153916-2c7772ba3064
	golang.org/x/net v0.0.0-20220403103023-749bd193bc2b
	golang.org/x/sync v0.0.0-20210220032951-036812b2e83c
	golang.org/x/sys v0.0.0-20220328115105-d36c6a25d886
	golang.zx2c4.com/wireguard v0.0.0-20220318042302-193cf8d6a5d6
	google.golang.org/grpc v1.45.0
	google.golang.org/protobuf v1.28.0
	gvisor.dev/gvisor v0.0.0
	h12.io/socks v1.0.3
	inet.af/netaddr v0.0.0-20211027220019-c74959edd3b6
	lukechampine.com/blake3 v1.1.7
)

// https://github.com/google/gvisor/releases/tag/release-20220314.0
replace gvisor.dev/gvisor => github.com/sagernet/gvisor v0.0.0-20220402114650-763d12dc953e

require (
	github.com/cheekybits/genny v1.0.0 // indirect
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/dgryski/go-metro v0.0.0-20211217172704-adc40b04c140 // indirect
	github.com/ebfe/bcrypt_pbkdf v0.0.0-20140212075826-3c8d2dcb253a // indirect
	github.com/ebfe/rc2 v0.0.0-20131011165748-24b9757f5521 // indirect
	github.com/fsnotify/fsnotify v1.5.1 // indirect
	github.com/go-playground/locales v0.14.0 // indirect
	github.com/go-playground/universal-translator v0.18.0 // indirect
	github.com/go-task/slim-sprig v0.0.0-20210107165309-348f09dbbbc0 // indirect
	github.com/google/btree v1.0.1 // indirect
	github.com/klauspost/cpuid/v2 v2.0.12 // indirect
	github.com/leodido/go-urn v1.2.1 // indirect
	github.com/lunixbochs/struc v0.0.0-20200707160740-784aaebc1d40 // indirect
	github.com/marten-seemann/qtls-go1-16 v0.1.5 // indirect
	github.com/marten-seemann/qtls-go1-18 v0.1.1 // indirect
	github.com/nxadm/tail v1.4.8 // indirect
	github.com/onsi/ginkgo v1.16.5 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/riobard/go-bloom v0.0.0-20200614022211-cdc8013cb5b3 // indirect
	github.com/xtaci/smux v1.5.16 // indirect
	go4.org/intern v0.0.0-20220301175310-a089fc204883 // indirect
	go4.org/unsafe/assume-no-moving-gc v0.0.0-20211027215541-db492cf91b37 // indirect
	golang.org/x/mod v0.6.0-dev.0.20220106191415-9b9b3d81d5e3 // indirect
	golang.org/x/text v0.3.7 // indirect
	golang.org/x/time v0.0.0-20220224211638-0e9765cccd65 // indirect
	golang.org/x/tools v0.1.10 // indirect
	golang.org/x/xerrors v0.0.0-20200804184101-5ec99f83aff1 // indirect
	golang.zx2c4.com/wintun v0.0.0-20211104114900-415007cec224 // indirect
	google.golang.org/genproto v0.0.0-20220322021311-435b647f9ef2 // indirect
	gopkg.in/tomb.v1 v1.0.0-20141024135613-dd632973f1e7 // indirect
	gopkg.in/yaml.v3 v3.0.0-20210107192922-496545a6307b // indirect
)
