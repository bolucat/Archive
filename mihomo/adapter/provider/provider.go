package provider

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"reflect"
	"runtime"
	"strings"
	"time"

	"github.com/metacubex/mihomo/adapter"
	"github.com/metacubex/mihomo/common/convert"
	"github.com/metacubex/mihomo/common/utils"
	mihomoHttp "github.com/metacubex/mihomo/component/http"
	"github.com/metacubex/mihomo/component/resource"
	C "github.com/metacubex/mihomo/constant"
	types "github.com/metacubex/mihomo/constant/provider"
	"github.com/metacubex/mihomo/tunnel/statistic"

	"github.com/dlclark/regexp2"
	"gopkg.in/yaml.v3"
)

const (
	ReservedName = "default"
)

type ProxySchema struct {
	Proxies []map[string]any `yaml:"proxies"`
}

// ProxySetProvider for auto gc
type ProxySetProvider struct {
	*proxySetProvider
}

type proxySetProvider struct {
	*resource.Fetcher[[]C.Proxy]
	proxies          []C.Proxy
	healthCheck      *HealthCheck
	version          uint32
	subscriptionInfo *SubscriptionInfo
}

func (pp *proxySetProvider) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]any{
		"name":             pp.Name(),
		"type":             pp.Type().String(),
		"vehicleType":      pp.VehicleType().String(),
		"proxies":          pp.Proxies(),
		"testUrl":          pp.healthCheck.url,
		"expectedStatus":   pp.healthCheck.expectedStatus.String(),
		"updatedAt":        pp.UpdatedAt(),
		"subscriptionInfo": pp.subscriptionInfo,
	})
}

func (pp *proxySetProvider) Version() uint32 {
	return pp.version
}

func (pp *proxySetProvider) Name() string {
	return pp.Fetcher.Name()
}

func (pp *proxySetProvider) HealthCheck() {
	pp.healthCheck.check()
}

func (pp *proxySetProvider) Update() error {
	_, _, err := pp.Fetcher.Update()
	return err
}

func (pp *proxySetProvider) Initial() error {
	_, err := pp.Fetcher.Initial()
	if err != nil {
		return err
	}
	pp.getSubscriptionInfo()
	pp.closeAllConnections()
	return nil
}

func (pp *proxySetProvider) Type() types.ProviderType {
	return types.Proxy
}

func (pp *proxySetProvider) Proxies() []C.Proxy {
	return pp.proxies
}

func (pp *proxySetProvider) Count() int {
	return len(pp.proxies)
}

func (pp *proxySetProvider) Touch() {
	pp.healthCheck.touch()
}

func (pp *proxySetProvider) HealthCheckURL() string {
	return pp.healthCheck.url
}

func (pp *proxySetProvider) RegisterHealthCheckTask(url string, expectedStatus utils.IntRanges[uint16], filter string, interval uint) {
	pp.healthCheck.registerHealthCheckTask(url, expectedStatus, filter, interval)
}

func (pp *proxySetProvider) setProxies(proxies []C.Proxy) {
	pp.proxies = proxies
	pp.healthCheck.setProxy(proxies)
	if pp.healthCheck.auto() {
		go pp.healthCheck.check()
	}
}

func (pp *proxySetProvider) getSubscriptionInfo() {
	if pp.VehicleType() != types.HTTP {
		return
	}
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second*90)
		defer cancel()
		resp, err := mihomoHttp.HttpRequestWithProxy(ctx, pp.Vehicle().Url(),
			http.MethodGet, nil, nil, pp.Vehicle().Proxy())
		if err != nil {
			return
		}
		defer resp.Body.Close()

		userInfoStr := strings.TrimSpace(resp.Header.Get("subscription-userinfo"))
		if userInfoStr == "" {
			resp2, err := mihomoHttp.HttpRequestWithProxy(ctx, pp.Vehicle().Url(),
				http.MethodGet, http.Header{"User-Agent": {"Quantumultx"}}, nil, pp.Vehicle().Proxy())
			if err != nil {
				return
			}
			defer resp2.Body.Close()
			userInfoStr = strings.TrimSpace(resp2.Header.Get("subscription-userinfo"))
			if userInfoStr == "" {
				return
			}
		}
		pp.subscriptionInfo = NewSubscriptionInfo(userInfoStr)
	}()
}

func (pp *proxySetProvider) closeAllConnections() {
	statistic.DefaultManager.Range(func(c statistic.Tracker) bool {
		for _, chain := range c.Chains() {
			if chain == pp.Name() {
				_ = c.Close()
				break
			}
		}
		return true
	})
}

func (pp *proxySetProvider) Close() error {
	pp.healthCheck.close()
	return pp.Fetcher.Close()
}

func NewProxySetProvider(name string, interval time.Duration, filter string, excludeFilter string, excludeType string, dialerProxy string, override OverrideSchema, vehicle types.Vehicle, hc *HealthCheck) (*ProxySetProvider, error) {
	excludeFilterReg, err := regexp2.Compile(excludeFilter, regexp2.None)
	if err != nil {
		return nil, fmt.Errorf("invalid excludeFilter regex: %w", err)
	}
	var excludeTypeArray []string
	if excludeType != "" {
		excludeTypeArray = strings.Split(excludeType, "|")
	}

	var filterRegs []*regexp2.Regexp
	for _, filter := range strings.Split(filter, "`") {
		filterReg, err := regexp2.Compile(filter, regexp2.None)
		if err != nil {
			return nil, fmt.Errorf("invalid filter regex: %w", err)
		}
		filterRegs = append(filterRegs, filterReg)
	}

	if hc.auto() {
		go hc.process()
	}

	pd := &proxySetProvider{
		proxies:     []C.Proxy{},
		healthCheck: hc,
	}

	fetcher := resource.NewFetcher[[]C.Proxy](name, interval, vehicle, proxiesParseAndFilter(filter, excludeFilter, excludeTypeArray, filterRegs, excludeFilterReg, dialerProxy, override), proxiesOnUpdate(pd))
	pd.Fetcher = fetcher
	wrapper := &ProxySetProvider{pd}
	runtime.SetFinalizer(wrapper, (*ProxySetProvider).Close)
	return wrapper, nil
}

func (pp *ProxySetProvider) Close() error {
	runtime.SetFinalizer(pp, nil)
	return pp.proxySetProvider.Close()
}

// CompatibleProvider for auto gc
type CompatibleProvider struct {
	*compatibleProvider
}

type compatibleProvider struct {
	name        string
	healthCheck *HealthCheck
	proxies     []C.Proxy
	version     uint32
}

func (cp *compatibleProvider) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]any{
		"name":           cp.Name(),
		"type":           cp.Type().String(),
		"vehicleType":    cp.VehicleType().String(),
		"proxies":        cp.Proxies(),
		"testUrl":        cp.healthCheck.url,
		"expectedStatus": cp.healthCheck.expectedStatus.String(),
	})
}

func (cp *compatibleProvider) Version() uint32 {
	return cp.version
}

func (cp *compatibleProvider) Name() string {
	return cp.name
}

func (cp *compatibleProvider) HealthCheck() {
	cp.healthCheck.check()
}

func (cp *compatibleProvider) Update() error {
	return nil
}

func (cp *compatibleProvider) Initial() error {
	if cp.healthCheck.interval != 0 && cp.healthCheck.url != "" {
		cp.HealthCheck()
	}
	return nil
}

func (cp *compatibleProvider) VehicleType() types.VehicleType {
	return types.Compatible
}

func (cp *compatibleProvider) Type() types.ProviderType {
	return types.Proxy
}

func (cp *compatibleProvider) Proxies() []C.Proxy {
	return cp.proxies
}

func (cp *compatibleProvider) Count() int {
	return len(cp.proxies)
}

func (cp *compatibleProvider) Touch() {
	cp.healthCheck.touch()
}

func (cp *compatibleProvider) HealthCheckURL() string {
	return cp.healthCheck.url
}

func (cp *compatibleProvider) RegisterHealthCheckTask(url string, expectedStatus utils.IntRanges[uint16], filter string, interval uint) {
	cp.healthCheck.registerHealthCheckTask(url, expectedStatus, filter, interval)
}

func (cp *compatibleProvider) Close() error {
	cp.healthCheck.close()
	return nil
}

func NewCompatibleProvider(name string, proxies []C.Proxy, hc *HealthCheck) (*CompatibleProvider, error) {
	if len(proxies) == 0 {
		return nil, errors.New("provider need one proxy at least")
	}

	if hc.auto() {
		go hc.process()
	}

	pd := &compatibleProvider{
		name:        name,
		proxies:     proxies,
		healthCheck: hc,
	}

	wrapper := &CompatibleProvider{pd}
	runtime.SetFinalizer(wrapper, (*CompatibleProvider).Close)
	return wrapper, nil
}

func (cp *CompatibleProvider) Close() error {
	runtime.SetFinalizer(cp, nil)
	return cp.compatibleProvider.Close()
}

func proxiesOnUpdate(pd *proxySetProvider) func([]C.Proxy) {
	return func(elm []C.Proxy) {
		pd.setProxies(elm)
		pd.version += 1
		pd.getSubscriptionInfo()
	}
}

func proxiesParseAndFilter(filter string, excludeFilter string, excludeTypeArray []string, filterRegs []*regexp2.Regexp, excludeFilterReg *regexp2.Regexp, dialerProxy string, override OverrideSchema) resource.Parser[[]C.Proxy] {
	return func(buf []byte) ([]C.Proxy, error) {
		schema := &ProxySchema{}

		if err := yaml.Unmarshal(buf, schema); err != nil {
			proxies, err1 := convert.ConvertsV2Ray(buf)
			if err1 != nil {
				return nil, fmt.Errorf("%w, %w", err, err1)
			}
			schema.Proxies = proxies
		}

		if schema.Proxies == nil {
			return nil, errors.New("file must have a `proxies` field")
		}

		proxies := []C.Proxy{}
		proxiesSet := map[string]struct{}{}
		for _, filterReg := range filterRegs {
			for idx, mapping := range schema.Proxies {
				if nil != excludeTypeArray && len(excludeTypeArray) > 0 {
					mType, ok := mapping["type"]
					if !ok {
						continue
					}
					pType, ok := mType.(string)
					if !ok {
						continue
					}
					flag := false
					for i := range excludeTypeArray {
						if strings.EqualFold(pType, excludeTypeArray[i]) {
							flag = true
							break
						}

					}
					if flag {
						continue
					}

				}
				mName, ok := mapping["name"]
				if !ok {
					continue
				}
				name, ok := mName.(string)
				if !ok {
					continue
				}
				if len(excludeFilter) > 0 {
					if mat, _ := excludeFilterReg.MatchString(name); mat {
						continue
					}
				}
				if len(filter) > 0 {
					if mat, _ := filterReg.MatchString(name); !mat {
						continue
					}
				}
				if _, ok := proxiesSet[name]; ok {
					continue
				}

				if len(dialerProxy) > 0 {
					mapping["dialer-proxy"] = dialerProxy
				}

				val := reflect.ValueOf(override)
				for i := 0; i < val.NumField(); i++ {
					field := val.Field(i)
					if field.IsNil() {
						continue
					}
					fieldName := strings.Split(val.Type().Field(i).Tag.Get("provider"), ",")[0]
					switch fieldName {
					case "additional-prefix":
						name := mapping["name"].(string)
						mapping["name"] = *field.Interface().(*string) + name
					case "additional-suffix":
						name := mapping["name"].(string)
						mapping["name"] = name + *field.Interface().(*string)
					case "proxy-name":
						// Iterate through all naming replacement rules and perform the replacements.
						for _, expr := range override.ProxyName {
							name := mapping["name"].(string)
							newName, err := expr.Pattern.Replace(name, expr.Target, 0, -1)
							if err != nil {
								return nil, fmt.Errorf("proxy name replace error: %w", err)
							}
							mapping["name"] = newName
						}
					default:
						mapping[fieldName] = field.Elem().Interface()
					}
				}

				proxy, err := adapter.ParseProxy(mapping)
				if err != nil {
					return nil, fmt.Errorf("proxy %d error: %w", idx, err)
				}

				proxiesSet[name] = struct{}{}
				proxies = append(proxies, proxy)
			}
		}

		if len(proxies) == 0 {
			if len(filter) > 0 {
				return nil, errors.New("doesn't match any proxy, please check your filter")
			}
			return nil, errors.New("file doesn't have any proxy")
		}

		return proxies, nil
	}
}
