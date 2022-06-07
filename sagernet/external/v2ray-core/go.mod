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
	github.com/go-playground/validator/v10 v10.11.0
	github.com/golang/mock v1.6.0
	github.com/golang/protobuf v1.5.2
	github.com/google/go-cmp v0.5.8
	github.com/gorilla/websocket v1.5.0
	github.com/jhump/protoreflect v1.12.0
	github.com/kierdavis/cfb8 v0.0.0-20180105024805-3a17c36ee2f8
	github.com/lucas-clemente/quic-go v0.27.1
	github.com/marten-seemann/qtls-go1-18 v0.1.2
	github.com/miekg/dns v1.1.49
	github.com/mustafaturan/bus v1.0.2
	github.com/pires/go-proxyproto v0.6.2
	github.com/sagernet/sing v0.0.0-20220606113732-5efacc1c7db5
	github.com/sagernet/sing-shadowsocks v0.0.0-20220607021139-f9c820eb0fc8
	github.com/seiflotfy/cuckoofilter v0.0.0-20220411075957-e3b120b3f5fb
	github.com/stretchr/testify v1.7.2
	github.com/v2fly/BrowserBridge v0.0.0-20210430233438-0570fc1d7d08
	github.com/v2fly/VSign v0.0.0-20201108000810-e2adc24bf848
	github.com/v2fly/ss-bloomring v0.0.0-20210312155135-28617310f63e
	github.com/xiaokangwang/VLite v0.0.0-20220418190619-cff95160a432
	github.com/xtls/go v0.0.0-20210920065950-d4af136d3672
	go.starlark.net v0.0.0-20220328144851-d1966c6b9fcd
	golang.org/x/crypto v0.0.0-20220525230936-793ad666bf5e
	golang.org/x/net v0.0.0-20220607020251-c690dde0001d
	golang.org/x/sync v0.0.0-20220601150217-0de741cfad7f
	golang.org/x/sys v0.0.0-20220520151302-bc2c85ada10a
	golang.zx2c4.com/wireguard v0.0.0-20220601130007-6a08d81f6bc4
	google.golang.org/grpc v1.47.0
	google.golang.org/protobuf v1.28.0
	gvisor.dev/gvisor v0.0.0
	h12.io/socks v1.0.3
	inet.af/netaddr v0.0.0-20211027220019-c74959edd3b6
	lukechampine.com/blake3 v1.1.7
)

// https://github.com/google/gvisor/releases/tag/release-20220314.0
replace gvisor.dev/gvisor => github.com/sagernet/gvisor v0.0.0-20220402114650-763d12dc953e

//replace github.com/sagernet/sing => ../sing

require (
	github.com/aead/cmac v0.0.0-20160719120800-7af84192f0b1 // indirect
	github.com/boljen/go-bitmap v0.0.0-20151001105940-23cd2fb0ce7d // indirect
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
	github.com/klauspost/cpuid v1.2.3 // indirect
	github.com/klauspost/cpuid/v2 v2.0.12 // indirect
	github.com/klauspost/reedsolomon v1.9.3 // indirect
	github.com/leodido/go-urn v1.2.1 // indirect
	github.com/lunixbochs/struc v0.0.0-20200707160740-784aaebc1d40 // indirect
	github.com/marten-seemann/qtls-go1-16 v0.1.5 // indirect
	github.com/marten-seemann/qtls-go1-17 v0.1.1 // indirect
	github.com/mustafaturan/monoton v1.0.0 // indirect
	github.com/nxadm/tail v1.4.8 // indirect
	github.com/onsi/ginkgo v1.16.5 // indirect
	github.com/patrickmn/go-cache v2.1.0+incompatible // indirect
	github.com/pion/dtls/v2 v2.0.0-rc.7 // indirect
	github.com/pion/logging v0.2.2 // indirect
	github.com/pion/sctp v1.7.6 // indirect
	github.com/pkg/errors v0.9.1 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/riobard/go-bloom v0.0.0-20200614022211-cdc8013cb5b3 // indirect
	github.com/secure-io/siv-go v0.0.0-20180922214919-5ff40651e2c4 // indirect
	github.com/xtaci/smux v1.5.16 // indirect
	go4.org/intern v0.0.0-20220301175310-a089fc204883 // indirect
	go4.org/unsafe/assume-no-moving-gc v0.0.0-20211027215541-db492cf91b37 // indirect
	golang.org/x/mod v0.6.0-dev.0.20220106191415-9b9b3d81d5e3 // indirect
	golang.org/x/text v0.3.7 // indirect
	golang.org/x/time v0.0.0-20220411224347-583f2d630306 // indirect
	golang.org/x/tools v0.1.11-0.20220325154526-54af36eca237 // indirect
	golang.org/x/xerrors v0.0.0-20200804184101-5ec99f83aff1 // indirect
	golang.zx2c4.com/wintun v0.0.0-20211104114900-415007cec224 // indirect
	google.golang.org/genproto v0.0.0-20220322021311-435b647f9ef2 // indirect
	gopkg.in/tomb.v1 v1.0.0-20141024135613-dd632973f1e7 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)
