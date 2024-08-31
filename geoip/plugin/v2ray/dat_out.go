package v2ray

import (
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/Loyalsoldier/geoip/lib"
	router "github.com/v2fly/v2ray-core/v5/app/router/routercommon"
	"github.com/v2fly/v2ray-core/v5/infra/conf/rule"
	"google.golang.org/protobuf/proto"
)

const (
	typeGeoIPdatOut = "v2rayGeoIPDat"
	descGeoIPdatOut = "Convert data to V2Ray GeoIP dat format"
)

var (
	defaultOutputName = "geoip.dat"
	defaultOutputDir  = filepath.Join("./", "output", "dat")
)

func init() {
	lib.RegisterOutputConfigCreator(typeGeoIPdatOut, func(action lib.Action, data json.RawMessage) (lib.OutputConverter, error) {
		return newGeoIPDat(action, data)
	})
	lib.RegisterOutputConverter(typeGeoIPdatOut, &geoIPDatOut{
		Description: descGeoIPdatOut,
	})
}

func newGeoIPDat(action lib.Action, data json.RawMessage) (lib.OutputConverter, error) {
	var tmp struct {
		OutputName     string     `json:"outputName"`
		OutputDir      string     `json:"outputDir"`
		Want           []string   `json:"wantedList"`
		OneFilePerList bool       `json:"oneFilePerList"`
		OnlyIPType     lib.IPType `json:"onlyIPType"`
	}

	if len(data) > 0 {
		if err := json.Unmarshal(data, &tmp); err != nil {
			return nil, err
		}
	}

	if tmp.OutputName == "" {
		tmp.OutputName = defaultOutputName
	}

	if tmp.OutputDir == "" {
		tmp.OutputDir = defaultOutputDir
	}

	// Filter want list
	wantList := make([]string, 0, len(tmp.Want))
	for _, want := range tmp.Want {
		if want = strings.ToUpper(strings.TrimSpace(want)); want != "" {
			wantList = append(wantList, want)
		}
	}

	return &geoIPDatOut{
		Type:           typeGeoIPdatOut,
		Action:         action,
		Description:    descGeoIPdatOut,
		OutputName:     tmp.OutputName,
		OutputDir:      tmp.OutputDir,
		Want:           wantList,
		OneFilePerList: tmp.OneFilePerList,
		OnlyIPType:     tmp.OnlyIPType,
	}, nil
}

type geoIPDatOut struct {
	Type           string
	Action         lib.Action
	Description    string
	OutputName     string
	OutputDir      string
	Want           []string
	OneFilePerList bool
	OnlyIPType     lib.IPType
}

func (g *geoIPDatOut) GetType() string {
	return g.Type
}

func (g *geoIPDatOut) GetAction() lib.Action {
	return g.Action
}

func (g *geoIPDatOut) GetDescription() string {
	return g.Description
}

func (g *geoIPDatOut) Output(container lib.Container) error {
	geoIPList := new(router.GeoIPList)
	geoIPList.Entry = make([]*router.GeoIP, 0, 300)
	updated := false

	switch len(g.Want) {
	case 0:
		list := make([]string, 0, 300)
		for entry := range container.Loop() {
			list = append(list, entry.GetName())
		}

		// Sort the list
		sort.Strings(list)

		for _, name := range list {
			entry, found := container.GetEntry(name)
			if !found {
				log.Printf("❌ entry %s not found", name)
				continue
			}
			geoIP, err := g.generateGeoIP(entry)
			if err != nil {
				return err
			}
			geoIPList.Entry = append(geoIPList.Entry, geoIP)
			updated = true

			if g.OneFilePerList {
				geoIPBytes, err := proto.Marshal(geoIPList)
				if err != nil {
					return err
				}
				filename := strings.ToLower(entry.GetName()) + ".dat"
				if err := g.writeFile(filename, geoIPBytes); err != nil {
					return err
				}
				geoIPList.Entry = nil
			}
		}

	default:
		// Sort the list
		sort.Strings(g.Want)

		for _, name := range g.Want {
			entry, found := container.GetEntry(name)
			if !found {
				log.Printf("❌ entry %s not found", name)
				continue
			}
			geoIP, err := g.generateGeoIP(entry)
			if err != nil {
				return err
			}
			geoIPList.Entry = append(geoIPList.Entry, geoIP)
			updated = true

			if g.OneFilePerList {
				geoIPBytes, err := proto.Marshal(geoIPList)
				if err != nil {
					return err
				}
				filename := strings.ToLower(entry.GetName()) + ".dat"
				if err := g.writeFile(filename, geoIPBytes); err != nil {
					return err
				}
				geoIPList.Entry = nil
			}
		}
	}

	if !g.OneFilePerList && updated {
		// Sort to make reproducible builds
		g.sort(geoIPList)

		geoIPBytes, err := proto.Marshal(geoIPList)
		if err != nil {
			return err
		}
		if err := g.writeFile(g.OutputName, geoIPBytes); err != nil {
			return err
		}
	}

	return nil
}

func (g *geoIPDatOut) generateGeoIP(entry *lib.Entry) (*router.GeoIP, error) {
	var entryCidr []string
	var err error
	switch g.OnlyIPType {
	case lib.IPv4:
		entryCidr, err = entry.MarshalText(lib.IgnoreIPv6)
	case lib.IPv6:
		entryCidr, err = entry.MarshalText(lib.IgnoreIPv4)
	default:
		entryCidr, err = entry.MarshalText()
	}
	if err != nil {
		return nil, err
	}

	v2rayCIDR := make([]*router.CIDR, 0, 1024)
	for _, cidrStr := range entryCidr {
		cidr, err := rule.ParseIP(cidrStr)
		if err != nil {
			return nil, err
		}
		v2rayCIDR = append(v2rayCIDR, cidr)
	}

	if len(v2rayCIDR) > 0 {
		return &router.GeoIP{
			CountryCode: entry.GetName(),
			Cidr:        v2rayCIDR,
		}, nil
	}

	return nil, fmt.Errorf("❌ [type %s | action %s] entry %s has no CIDR", g.Type, g.Action, entry.GetName())
}

// Sort by country code to make reproducible builds
func (g *geoIPDatOut) sort(list *router.GeoIPList) {
	sort.SliceStable(list.Entry, func(i, j int) bool {
		return list.Entry[i].CountryCode < list.Entry[j].CountryCode
	})
}

func (g *geoIPDatOut) writeFile(filename string, geoIPBytes []byte) error {
	if err := os.MkdirAll(g.OutputDir, 0755); err != nil {
		return err
	}

	if err := os.WriteFile(filepath.Join(g.OutputDir, filename), geoIPBytes, 0644); err != nil {
		return err
	}

	log.Printf("✅ [%s] %s --> %s", g.Type, filename, g.OutputDir)

	return nil
}
