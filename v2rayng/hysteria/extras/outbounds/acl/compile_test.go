package acl

import (
	"net"
	"testing"

	"github.com/apernet/hysteria/extras/v2/outbounds/acl/v2geo"

	"github.com/stretchr/testify/assert"
)

var _ GeoLoader = (*testGeoLoader)(nil)

type testGeoLoader struct{}

func (l *testGeoLoader) LoadGeoIP() (map[string]*v2geo.GeoIP, error) {
	return v2geo.LoadGeoIP("v2geo/geoip.dat")
}

func (l *testGeoLoader) LoadGeoSite() (map[string]*v2geo.GeoSite, error) {
	return v2geo.LoadGeoSite("v2geo/geosite.dat")
}

func TestCompile(t *testing.T) {
	ob1, ob2, ob3, ob4, ob5, ob6 := 1, 2, 3, 4, 5, 6
	rules := []TextRule{
		{
			Outbound:      "ob1",
			Address:       "1.2.3.4",
			ProtoPort:     "",
			HijackAddress: "",
		},
		{
			Outbound:      "ob2",
			Address:       "8.8.8.0/24",
			ProtoPort:     "*",
			HijackAddress: "1.1.1.1",
		},
		{
			Outbound:      "ob3",
			Address:       "all",
			ProtoPort:     "udp/443",
			HijackAddress: "",
		},
		{
			Outbound:      "ob1",
			Address:       "2606:4700::6810:85e5",
			ProtoPort:     "tcp",
			HijackAddress: "2606:4700::6810:85e6",
		},
		{
			Outbound:      "ob2",
			Address:       "2606:4700::/44",
			ProtoPort:     "*/8888",
			HijackAddress: "",
		},
		{
			Outbound:      "ob3",
			Address:       "*.v2ex.com",
			ProtoPort:     "udp",
			HijackAddress: "",
		},
		{
			Outbound:      "ob1",
			Address:       "crap.v2ex.com",
			ProtoPort:     "tcp/80",
			HijackAddress: "2.2.2.2",
		},
		{
			Outbound:      "ob2",
			Address:       "geoip:JP",
			ProtoPort:     "*/*",
			HijackAddress: "",
		},
		{
			Outbound:      "ob4",
			Address:       "geosite:4chan",
			ProtoPort:     "*/*",
			HijackAddress: "",
		},
		{
			Outbound:      "ob4",
			Address:       "geosite:google @cn",
			ProtoPort:     "*/*",
			HijackAddress: "",
		},
		{
			Outbound:      "ob5",
			Address:       "suffix:microsoft.com",
			ProtoPort:     "*/*",
			HijackAddress: "",
		},
		{
			Outbound:      "ob6",
			Address:       "all",
			ProtoPort:     "tcp/6881-6889",
			HijackAddress: "",
		},
	}
	comp, err := Compile[int](rules, map[string]int{
		"ob1": ob1,
		"ob2": ob2,
		"ob3": ob3,
		"ob4": ob4,
		"ob5": ob5,
		"ob6": ob6,
	}, 100, &testGeoLoader{})
	assert.NoError(t, err)

	tests := []struct {
		host         HostInfo
		proto        Protocol
		port         uint16
		wantOutbound int
		wantIP       net.IP
	}{
		{
			host: HostInfo{
				IPv4: net.ParseIP("1.2.3.4"),
			},
			proto:        ProtocolTCP,
			port:         1234,
			wantOutbound: ob1,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				IPv4: net.ParseIP("8.8.8.4"),
			},
			proto:        ProtocolUDP,
			port:         5353,
			wantOutbound: ob2,
			wantIP:       net.ParseIP("1.1.1.1"),
		},
		{
			host: HostInfo{
				Name: "lean.delicious.com",
			},
			proto:        ProtocolUDP,
			port:         443,
			wantOutbound: ob3,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				IPv6: net.ParseIP("2606:4700::6810:85e5"),
			},
			proto:        ProtocolTCP,
			port:         80,
			wantOutbound: ob1,
			wantIP:       net.ParseIP("2606:4700::6810:85e6"),
		},
		{
			host: HostInfo{
				IPv6: net.ParseIP("2606:4700:0:0:0:0:0:1"),
			},
			proto:        ProtocolUDP,
			port:         8888,
			wantOutbound: ob2,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "www.v2ex.com",
			},
			proto:        ProtocolUDP,
			port:         1234,
			wantOutbound: ob3,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "crap.v2ex.com",
			},
			proto:        ProtocolTCP,
			port:         80,
			wantOutbound: ob1,
			wantIP:       net.ParseIP("2.2.2.2"),
		},
		{
			host: HostInfo{
				IPv4: net.ParseIP("210.140.92.187"),
			},
			proto:        ProtocolTCP,
			port:         25,
			wantOutbound: ob2,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				IPv4: net.ParseIP("175.45.176.73"),
			},
			proto:        ProtocolTCP,
			port:         80,
			wantOutbound: 0, // no match default
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "boards.4channel.org",
			},
			proto:        ProtocolTCP,
			port:         443,
			wantOutbound: ob4,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "gstatic-cn.com",
			},
			proto:        ProtocolUDP,
			port:         9999,
			wantOutbound: ob4,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "hoho.waymo.com",
			},
			proto:        ProtocolUDP,
			port:         9999,
			wantOutbound: 0, // no match default
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "microsoft.com",
			},
			proto:        ProtocolTCP,
			port:         6000,
			wantOutbound: ob5,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "real.microsoft.com",
			},
			proto:        ProtocolUDP,
			port:         5353,
			wantOutbound: ob5,
			wantIP:       nil,
		},
		{
			host: HostInfo{
				Name: "fakemicrosoft.com",
			},
			proto:        ProtocolTCP,
			port:         5000,
			wantOutbound: 0, // no match default
			wantIP:       nil,
		},
		{
			host: HostInfo{
				IPv4: net.ParseIP("223.1.1.1"),
			},
			proto:        ProtocolTCP,
			port:         6883,
			wantOutbound: ob6, // match range port rule 6881-6889
			wantIP:       nil,
		},
	}

	for _, test := range tests {
		gotOutbound, gotIP := comp.Match(test.host, test.proto, test.port)
		assert.Equal(t, test.wantOutbound, gotOutbound)
		assert.Equal(t, test.wantIP, gotIP)
	}

	// Test Invalid Port Range Rule
	eb1 := 1
	invalidRules := []TextRule{
		{
			Outbound:      "eb1",
			Address:       "1.1.2.0/24",
			ProtoPort:     "*/3-1",
			HijackAddress: "",
		},
	}

	_, err = Compile[int](invalidRules, map[string]int{
		"eb1": eb1,
	}, 100, &testGeoLoader{})
	assert.Error(t, err)
}

func Test_parseGeoSiteName(t *testing.T) {
	tests := []struct {
		name  string
		s     string
		want  string
		want1 []string
	}{
		{
			name:  "no attrs",
			s:     "pornhub",
			want:  "pornhub",
			want1: []string{},
		},
		{
			name:  "one attr 1",
			s:     "xiaomi@cn",
			want:  "xiaomi",
			want1: []string{"cn"},
		},
		{
			name:  "one attr 2",
			s:     " google @jp ",
			want:  "google",
			want1: []string{"jp"},
		},
		{
			name:  "two attrs 1",
			s:     "netflix@jp@kr",
			want:  "netflix",
			want1: []string{"jp", "kr"},
		},
		{
			name:  "two attrs 2",
			s:     "netflix @xixi    @haha ",
			want:  "netflix",
			want1: []string{"xixi", "haha"},
		},
		{
			name:  "empty",
			s:     "",
			want:  "",
			want1: []string{},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, got1 := parseGeoSiteName(tt.s)
			assert.Equalf(t, tt.want, got, "parseGeoSiteName(%v)", tt.s)
			assert.Equalf(t, tt.want1, got1, "parseGeoSiteName(%v)", tt.s)
		})
	}
}
