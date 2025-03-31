package option

import (
	"net/netip"
	"net/url"
	"reflect"

	"github.com/sagernet/sing/common/json"
	"github.com/sagernet/sing/common/json/badoption"
	M "github.com/sagernet/sing/common/metadata"
)

type TailscaleEndpointOptions struct {
	DialerOptions
	StateDirectory         string           `json:"state_directory,omitempty"`
	AuthKey                string           `json:"auth_key,omitempty"`
	ControlURL             string           `json:"control_url,omitempty"`
	Ephemeral              bool             `json:"ephemeral,omitempty"`
	Hostname               string           `json:"hostname,omitempty"`
	ExitNode               string           `json:"exit_node,omitempty"`
	ExitNodeAllowLANAccess bool             `json:"exit_node_allow_lan_access,omitempty"`
	AdvertiseRoutes        []netip.Prefix   `json:"advertise_routes,omitempty"`
	AdvertiseExitNode      bool             `json:"advertise_exit_node,omitempty"`
	UDPTimeout             UDPTimeoutCompat `json:"udp_timeout,omitempty"`
}

type TailscaleDNSServerOptions struct {
	Endpoint               string `json:"endpoint,omitempty"`
	AcceptDefaultResolvers bool   `json:"accept_default_resolvers,omitempty"`
}

type DERPServiceOptions struct {
	ListenOptions
	InboundTLSOptionsContainer
	ConfigPath           string                                         `json:"config_path,omitempty"`
	VerifyClientEndpoint badoption.Listable[string]                     `json:"verify_client_endpoint,omitempty"`
	VerifyClientURL      badoption.Listable[DERPVerifyClientURLOptions] `json:"verify_client_url,omitempty"`
	MeshWith             badoption.Listable[DERPMeshOptions]            `json:"mesh_with,omitempty"`
	MeshPSK              string                                         `json:"mesh_psk,omitempty"`
	MeshPSKFile          string                                         `json:"mesh_psk_file,omitempty"`
	DomainResolver       *DomainResolveOptions                          `json:"domain_resolver,omitempty"`
}

type _DERPVerifyClientURLOptions struct {
	URL string `json:"url,omitempty"`
	DialerOptions
}

type DERPVerifyClientURLOptions _DERPVerifyClientURLOptions

func (d DERPVerifyClientURLOptions) ServerIsDomain() bool {
	verifyURL, err := url.Parse(d.URL)
	if err != nil {
		return false
	}
	return M.IsDomainName(verifyURL.Host)
}

func (d DERPVerifyClientURLOptions) MarshalJSON() ([]byte, error) {
	if reflect.DeepEqual(d, _DERPVerifyClientURLOptions{}) {
		return json.Marshal(d.URL)
	} else {
		return json.Marshal(_DERPVerifyClientURLOptions(d))
	}
}

func (d *DERPVerifyClientURLOptions) UnmarshalJSON(bytes []byte) error {
	var stringValue string
	err := json.Unmarshal(bytes, &stringValue)
	if err == nil {
		d.URL = stringValue
		return nil
	}
	return json.Unmarshal(bytes, (*_DERPVerifyClientURLOptions)(d))
}

type DERPMeshOptions struct {
	ServerOptions
	Host string `json:"host,omitempty"`
	OutboundTLSOptionsContainer
	DialerOptions
}

type DERPSTUNServiceOptions struct {
	ListenOptions
}
