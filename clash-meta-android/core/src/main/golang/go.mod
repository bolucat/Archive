module cfa

go 1.20

require (
	github.com/Kr328/tun2socket v0.0.0-20220414050025-d07c78d06d34
	github.com/dlclark/regexp2 v1.11.4
	github.com/metacubex/mihomo v1.7.0
	github.com/miekg/dns v1.1.62
	github.com/oschwald/maxminddb-golang v1.12.0
	golang.org/x/sync v0.8.0
	gopkg.in/yaml.v2 v2.4.0
)

replace github.com/metacubex/mihomo => ../../foss/golang/clash

replace github.com/sagernet/sing => github.com/metacubex/sing v0.0.0-20240724044459-6f3cf5896297

require (
	github.com/3andne/restls-client-go v0.1.6 // indirect
	github.com/RyuaNerin/go-krypto v1.2.4 // indirect
	github.com/Yawning/aez v0.0.0-20211027044916-e49e68abd344 // indirect
	github.com/andybalholm/brotli v1.0.6 // indirect
	github.com/bahlo/generic-list-go v0.2.0 // indirect
	github.com/buger/jsonparser v1.1.1 // indirect
	github.com/cloudflare/circl v1.3.7 // indirect
	github.com/coreos/go-iptables v0.7.0 // indirect
	github.com/ericlagergren/aegis v0.0.0-20230312195928-b4ce538b56f9 // indirect
	github.com/ericlagergren/polyval v0.0.0-20220411101811-e25bc10ba391 // indirect
	github.com/ericlagergren/siv v0.0.0-20220507050439-0b757b3aa5f1 // indirect
	github.com/ericlagergren/subtle v0.0.0-20220507045147-890d697da010 // indirect
	github.com/fsnotify/fsnotify v1.7.0 // indirect
	github.com/gaukas/godicttls v0.0.4 // indirect
	github.com/go-ole/go-ole v1.3.0 // indirect
	github.com/go-task/slim-sprig v0.0.0-20230315185526-52ccab3ef572 // indirect
	github.com/gobwas/httphead v0.1.0 // indirect
	github.com/gobwas/pool v0.2.1 // indirect
	github.com/gobwas/ws v1.4.0 // indirect
	github.com/gofrs/uuid/v5 v5.3.0 // indirect
	github.com/google/btree v1.1.2 // indirect
	github.com/google/go-cmp v0.6.0 // indirect
	github.com/google/pprof v0.0.0-20210407192527-94a9f03dee38 // indirect
	github.com/hashicorp/yamux v0.1.1 // indirect
	github.com/insomniacslk/dhcp v0.0.0-20240812123929-b105c29bd1b5 // indirect
	github.com/josharian/native v1.1.0 // indirect
	github.com/klauspost/compress v1.17.9 // indirect
	github.com/klauspost/cpuid/v2 v2.2.8 // indirect
	github.com/lufia/plan9stats v0.0.0-20211012122336-39d0f177ccd0 // indirect
	github.com/lunixbochs/struc v0.0.0-20200707160740-784aaebc1d40 // indirect
	github.com/mailru/easyjson v0.7.7 // indirect
	github.com/mdlayher/netlink v1.7.2 // indirect
	github.com/mdlayher/socket v0.4.1 // indirect
	github.com/metacubex/bbolt v0.0.0-20240822011022-aed6d4850399 // indirect
	github.com/metacubex/chacha v0.1.0 // indirect
	github.com/metacubex/gopacket v1.1.20-0.20230608035415-7e2f98a3e759 // indirect
	github.com/metacubex/gvisor v0.0.0-20240320004321-933faba989ec // indirect
	github.com/metacubex/quic-go v0.46.1-0.20240807232329-1c6cb2d67f58 // indirect
	github.com/metacubex/randv2 v0.2.0 // indirect
	github.com/metacubex/sing-quic v0.0.0-20240827003841-cd97758ed8b4 // indirect
	github.com/metacubex/sing-shadowsocks v0.2.8 // indirect
	github.com/metacubex/sing-shadowsocks2 v0.2.2 // indirect
	github.com/metacubex/sing-tun v0.2.7-0.20240729131039-ed03f557dee1 // indirect
	github.com/metacubex/sing-vmess v0.1.9-0.20240719134745-1df6fb20bbf9 // indirect
	github.com/metacubex/sing-wireguard v0.0.0-20240826061955-1e4e67afe5cd // indirect
	github.com/metacubex/tfo-go v0.0.0-20240830120620-c5e019b67785 // indirect
	github.com/metacubex/utls v1.6.6 // indirect
	github.com/mroth/weightedrand/v2 v2.1.0 // indirect
	github.com/oasisprotocol/deoxysii v0.0.0-20220228165953-2091330c22b7 // indirect
	github.com/onsi/ginkgo/v2 v2.9.5 // indirect
	github.com/openacid/low v0.1.21 // indirect
	github.com/pierrec/lz4/v4 v4.1.14 // indirect
	github.com/power-devops/perfstat v0.0.0-20210106213030-5aafc221ea8c // indirect
	github.com/puzpuzpuz/xsync/v3 v3.4.0 // indirect
	github.com/quic-go/qpack v0.4.0 // indirect
	github.com/quic-go/qtls-go1-20 v0.4.1 // indirect
	github.com/sagernet/fswatch v0.1.1 // indirect
	github.com/sagernet/netlink v0.0.0-20240612041022-b9a21c07ac6a // indirect
	github.com/sagernet/nftables v0.3.0-beta.4 // indirect
	github.com/sagernet/sing v0.5.0-alpha.13 // indirect
	github.com/sagernet/sing-mux v0.2.1-0.20240124034317-9bfb33698bb6 // indirect
	github.com/sagernet/sing-shadowtls v0.1.4 // indirect
	github.com/sagernet/smux v0.0.0-20231208180855-7041f6ea79e7 // indirect
	github.com/sagernet/wireguard-go v0.0.0-20231209092712-9a439356a62e // indirect
	github.com/samber/lo v1.47.0 // indirect
	github.com/shirou/gopsutil/v3 v3.24.5 // indirect
	github.com/shoenig/go-m1cpu v0.1.6 // indirect
	github.com/sina-ghaderi/poly1305 v0.0.0-20220724002748-c5926b03988b // indirect
	github.com/sina-ghaderi/rabaead v0.0.0-20220730151906-ab6e06b96e8c // indirect
	github.com/sina-ghaderi/rabbitio v0.0.0-20220730151941-9ce26f4f872e // indirect
	github.com/sirupsen/logrus v1.9.3 // indirect
	github.com/tklauser/go-sysconf v0.3.12 // indirect
	github.com/tklauser/numcpus v0.6.1 // indirect
	github.com/u-root/uio v0.0.0-20230220225925-ffce2a382923 // indirect
	github.com/vishvananda/netns v0.0.4 // indirect
	github.com/wk8/go-ordered-map/v2 v2.1.8 // indirect
	github.com/yusufpapurcu/wmi v1.2.4 // indirect
	gitlab.com/go-extension/aes-ccm v0.0.0-20230221065045-e58665ef23c7 // indirect
	gitlab.com/yawning/bsaes.git v0.0.0-20190805113838-0a714cd429ec // indirect
	go.uber.org/mock v0.4.0 // indirect
	go4.org/netipx v0.0.0-20231129151722-fdeea329fbba // indirect
	golang.org/x/crypto v0.26.0 // indirect
	golang.org/x/exp v0.0.0-20240808152545-0cdaa3abc0fa // indirect
	golang.org/x/mod v0.20.0 // indirect
	golang.org/x/net v0.28.0 // indirect
	golang.org/x/sys v0.24.0 // indirect
	golang.org/x/text v0.17.0 // indirect
	golang.org/x/time v0.5.0 // indirect
	golang.org/x/tools v0.24.0 // indirect
	google.golang.org/protobuf v1.34.2 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
	lukechampine.com/blake3 v1.3.0 // indirect
)
