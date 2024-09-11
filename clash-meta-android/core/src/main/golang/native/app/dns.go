package app

import (
	"strings"

	"github.com/metacubex/mihomo/dns"
)

func NotifyDnsChanged(dnsList string) {
	dns.UpdateSystemDNS(strings.Split(dnsList, ","))
	dns.FlushCacheWithDefaultResolver()
}
