package dns

import (
	"net/netip"

	"github.com/metacubex/mihomo/common/lru"
	"github.com/metacubex/mihomo/component/fakeip"
	C "github.com/metacubex/mihomo/constant"
)

type ResolverEnhancer struct {
	mode          C.DNSMode
	fakeIPPool    *fakeip.Pool
	fakeIPPool6   *fakeip.Pool
	fakeIPSkipper *fakeip.Skipper
	mapping       *lru.LruCache[netip.Addr, string]
	useHosts      bool
}

func (h *ResolverEnhancer) FakeIPEnabled() bool {
	return h.mode == C.DNSFakeIP
}

func (h *ResolverEnhancer) MappingEnabled() bool {
	return h.mode == C.DNSFakeIP || h.mode == C.DNSMapping
}

func (h *ResolverEnhancer) IsExistFakeIP(ip netip.Addr) bool {
	if !h.FakeIPEnabled() {
		return false
	}

	if pool := h.fakeIPPool; pool != nil {
		return pool.Exist(ip)
	}

	if pool6 := h.fakeIPPool6; pool6 != nil {
		return pool6.Exist(ip)
	}

	return false
}

func (h *ResolverEnhancer) IsFakeIP(ip netip.Addr) bool {
	if !h.FakeIPEnabled() {
		return false
	}

	if pool := h.fakeIPPool; pool != nil {
		return pool.IPNet().Contains(ip) && ip != pool.Gateway() && ip != pool.Broadcast()
	}

	if pool6 := h.fakeIPPool6; pool6 != nil {
		return pool6.IPNet().Contains(ip) && ip != pool6.Gateway() && ip != pool6.Broadcast()
	}

	return false
}

func (h *ResolverEnhancer) IsFakeBroadcastIP(ip netip.Addr) bool {
	if !h.FakeIPEnabled() {
		return false
	}

	if pool := h.fakeIPPool; pool != nil {
		return pool.Broadcast() == ip
	}

	if pool6 := h.fakeIPPool6; pool6 != nil {
		return pool6.Broadcast() == ip
	}

	return false
}

func (h *ResolverEnhancer) FindHostByIP(ip netip.Addr) (string, bool) {
	if pool := h.fakeIPPool; pool != nil {
		if host, existed := pool.LookBack(ip); existed {
			return host, true
		}
	}

	if pool6 := h.fakeIPPool6; pool6 != nil {
		if host, existed := pool6.LookBack(ip); existed {
			return host, true
		}
	}

	if mapping := h.mapping; mapping != nil {
		if host, existed := h.mapping.Get(ip); existed {
			return host, true
		}
	}

	return "", false
}

func (h *ResolverEnhancer) InsertHostByIP(ip netip.Addr, host string) {
	if mapping := h.mapping; mapping != nil {
		h.mapping.Set(ip, host)
	}
}

func (h *ResolverEnhancer) FlushFakeIP() error {
	if pool := h.fakeIPPool; pool != nil {
		return pool.FlushFakeIP()
	}
	if pool6 := h.fakeIPPool6; pool6 != nil {
		return pool6.FlushFakeIP()
	}
	return nil
}

func (h *ResolverEnhancer) PatchFrom(o *ResolverEnhancer) {
	if h.mapping != nil && o.mapping != nil {
		o.mapping.CloneTo(h.mapping)
	}

	if h.fakeIPPool != nil && o.fakeIPPool != nil {
		h.fakeIPPool.CloneFrom(o.fakeIPPool)
	}

	if h.fakeIPPool6 != nil && o.fakeIPPool6 != nil {
		h.fakeIPPool6.CloneFrom(o.fakeIPPool6)
	}
}

func (h *ResolverEnhancer) StoreFakePoolState() {
	if h.fakeIPPool != nil {
		h.fakeIPPool.StoreState()
	}

	if h.fakeIPPool6 != nil {
		h.fakeIPPool6.StoreState()
	}
}

type EnhancerConfig struct {
	IPv6          bool
	EnhancedMode  C.DNSMode
	FakeIPPool    *fakeip.Pool
	FakeIPPool6   *fakeip.Pool
	FakeIPSkipper *fakeip.Skipper
	UseHosts      bool
}

func NewEnhancer(cfg EnhancerConfig) *ResolverEnhancer {
	e := &ResolverEnhancer{
		mode:     cfg.EnhancedMode,
		useHosts: cfg.UseHosts,
	}

	if cfg.EnhancedMode != C.DNSNormal {
		e.fakeIPPool = cfg.FakeIPPool
		if cfg.IPv6 {
			e.fakeIPPool6 = cfg.FakeIPPool6
		}
		e.fakeIPSkipper = cfg.FakeIPSkipper
		e.mapping = lru.New(lru.WithSize[netip.Addr, string](4096))
	}

	return e
}
