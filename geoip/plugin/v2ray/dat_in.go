package v2ray

import (
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"strings"

	"github.com/Loyalsoldier/geoip/lib"
	router "github.com/v2fly/v2ray-core/v5/app/router/routercommon"
	"google.golang.org/protobuf/proto"
)

const (
	typeGeoIPdatIn = "v2rayGeoIPDat"
	descGeoIPdatIn = "Convert V2Ray GeoIP dat to other formats"
)

func init() {
	lib.RegisterInputConfigCreator(typeGeoIPdatIn, func(action lib.Action, data json.RawMessage) (lib.InputConverter, error) {
		return newGeoIPDatIn(action, data)
	})
	lib.RegisterInputConverter(typeGeoIPdatIn, &geoIPDatIn{
		Description: descGeoIPdatIn,
	})
}

func newGeoIPDatIn(action lib.Action, data json.RawMessage) (lib.InputConverter, error) {
	var tmp struct {
		URI        string     `json:"uri"`
		Want       []string   `json:"wantedList"`
		OnlyIPType lib.IPType `json:"onlyIPType"`
	}

	if len(data) > 0 {
		if err := json.Unmarshal(data, &tmp); err != nil {
			return nil, err
		}
	}

	if tmp.URI == "" {
		return nil, fmt.Errorf("[type %s | action %s] uri must be specified in config", typeGeoIPdatIn, action)
	}

	// Filter want list
	wantList := make(map[string]bool)
	for _, want := range tmp.Want {
		if want = strings.ToUpper(strings.TrimSpace(want)); want != "" {
			wantList[want] = true
		}
	}

	return &geoIPDatIn{
		Type:        typeGeoIPdatIn,
		Action:      action,
		Description: descGeoIPdatIn,
		URI:         tmp.URI,
		Want:        wantList,
		OnlyIPType:  tmp.OnlyIPType,
	}, nil
}

type geoIPDatIn struct {
	Type        string
	Action      lib.Action
	Description string
	URI         string
	Want        map[string]bool
	OnlyIPType  lib.IPType
}

func (g *geoIPDatIn) GetType() string {
	return g.Type
}

func (g *geoIPDatIn) GetAction() lib.Action {
	return g.Action
}

func (g *geoIPDatIn) GetDescription() string {
	return g.Description
}

func (g *geoIPDatIn) Input(container lib.Container) (lib.Container, error) {
	entries := make(map[string]*lib.Entry)
	var err error

	switch {
	case strings.HasPrefix(strings.ToLower(g.URI), "http://"), strings.HasPrefix(strings.ToLower(g.URI), "https://"):
		err = g.walkRemoteFile(g.URI, entries)
	default:
		err = g.walkLocalFile(g.URI, entries)
	}

	if err != nil {
		return nil, err
	}

	if len(entries) == 0 {
		return nil, fmt.Errorf("❌ [type %s | action %s] no entry is generated", typeGeoIPdatIn, g.Action)
	}

	var ignoreIPType lib.IgnoreIPOption
	switch g.OnlyIPType {
	case lib.IPv4:
		ignoreIPType = lib.IgnoreIPv6
	case lib.IPv6:
		ignoreIPType = lib.IgnoreIPv4
	}

	for _, entry := range entries {
		switch g.Action {
		case lib.ActionAdd:
			if err := container.Add(entry, ignoreIPType); err != nil {
				return nil, err
			}
		case lib.ActionRemove:
			if err := container.Remove(entry, lib.CaseRemovePrefix, ignoreIPType); err != nil {
				return nil, err
			}
		default:
			return nil, lib.ErrUnknownAction
		}
	}

	return container, nil
}

func (g *geoIPDatIn) walkLocalFile(path string, entries map[string]*lib.Entry) error {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()

	if err := g.generateEntries(file, entries); err != nil {
		return err
	}

	return nil
}

func (g *geoIPDatIn) walkRemoteFile(url string, entries map[string]*lib.Entry) error {
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return fmt.Errorf("failed to get remote file %s, http status code %d", url, resp.StatusCode)
	}

	if err := g.generateEntries(resp.Body, entries); err != nil {
		return err
	}

	return nil
}

func (g *geoIPDatIn) generateEntries(reader io.Reader, entries map[string]*lib.Entry) error {
	geoipBytes, err := io.ReadAll(reader)
	if err != nil {
		return err
	}

	var geoipList router.GeoIPList
	if err := proto.Unmarshal(geoipBytes, &geoipList); err != nil {
		return err
	}

	for _, geoip := range geoipList.Entry {
		name := strings.ToUpper(strings.TrimSpace(geoip.CountryCode))

		if len(g.Want) > 0 && !g.Want[name] {
			continue
		}

		entry, found := entries[name]
		if !found {
			entry = lib.NewEntry(name)
		}

		for _, v2rayCIDR := range geoip.Cidr {
			ipStr := net.IP(v2rayCIDR.GetIp()).String() + "/" + fmt.Sprint(v2rayCIDR.GetPrefix())
			if err := entry.AddPrefix(ipStr); err != nil {
				return err
			}
		}

		entries[name] = entry
	}

	return nil
}
