package mihomo

import (
	"encoding/binary"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/Loyalsoldier/geoip/lib"
	"github.com/klauspost/compress/zstd"
	"go4.org/netipx"
)

const (
	typeMRSOut = "mihomoMRS"
	descMRSOut = "Convert data to mihomo MRS format"
)

var (
	defaultOutputDir = filepath.Join("./", "output", "mrs")
)

func init() {
	lib.RegisterOutputConfigCreator(typeMRSOut, func(action lib.Action, data json.RawMessage) (lib.OutputConverter, error) {
		return newMRSOut(action, data)
	})
	lib.RegisterOutputConverter(typeMRSOut, &mrsOut{
		Description: descMRSOut,
	})
}

func newMRSOut(action lib.Action, data json.RawMessage) (lib.OutputConverter, error) {
	var tmp struct {
		OutputDir  string     `json:"outputDir"`
		Want       []string   `json:"wantedList"`
		OnlyIPType lib.IPType `json:"onlyIPType"`
	}

	if len(data) > 0 {
		if err := json.Unmarshal(data, &tmp); err != nil {
			return nil, err
		}
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

	return &mrsOut{
		Type:        typeMRSOut,
		Action:      action,
		Description: descMRSOut,
		OutputDir:   tmp.OutputDir,
		Want:        wantList,
		OnlyIPType:  tmp.OnlyIPType,
	}, nil
}

type mrsOut struct {
	Type        string
	Action      lib.Action
	Description string
	OutputDir   string
	Want        []string
	OnlyIPType  lib.IPType
}

func (m *mrsOut) GetType() string {
	return m.Type
}

func (m *mrsOut) GetAction() lib.Action {
	return m.Action
}

func (m *mrsOut) GetDescription() string {
	return m.Description
}

func (m *mrsOut) Output(container lib.Container) error {
	switch len(m.Want) {
	case 0:
		list := make([]string, 0, 300)
		for entry := range container.Loop() {
			list = append(list, entry.GetName())
		}

		// Sort the list
		slices.Sort(list)

		for _, name := range list {
			entry, found := container.GetEntry(name)
			if !found {
				log.Printf("❌ entry %s not found", name)
				continue
			}
			if err := m.generate(entry); err != nil {
				return err
			}
		}

	default:
		// Sort the list
		slices.Sort(m.Want)

		for _, name := range m.Want {
			entry, found := container.GetEntry(name)
			if !found {
				log.Printf("❌ entry %s not found", name)
				continue
			}

			if err := m.generate(entry); err != nil {
				return err
			}
		}
	}

	return nil
}

func (m *mrsOut) generate(entry *lib.Entry) error {
	var ipRanges []netipx.IPRange
	var err error
	switch m.OnlyIPType {
	case lib.IPv4:
		ipRanges, err = entry.MarshalIPRange(lib.IgnoreIPv6)
	case lib.IPv6:
		ipRanges, err = entry.MarshalIPRange(lib.IgnoreIPv4)
	default:
		ipRanges, err = entry.MarshalIPRange()
	}
	if err != nil {
		return err
	}

	if len(ipRanges) == 0 {
		return fmt.Errorf("❌ [type %s | action %s] entry %s has no CIDR", m.Type, m.Action, entry.GetName())
	}

	filename := strings.ToLower(entry.GetName()) + ".mrs"
	if err := m.writeFile(filename, ipRanges); err != nil {
		return err
	}

	return nil
}

func (m *mrsOut) writeFile(filename string, ipRanges []netipx.IPRange) error {
	if err := os.MkdirAll(m.OutputDir, 0755); err != nil {
		return err
	}

	f, err := os.Create(filepath.Join(m.OutputDir, filename))
	if err != nil {
		return err
	}
	defer f.Close()

	err = m.convertToMrs(ipRanges, f)
	if err != nil {
		return err
	}

	log.Printf("✅ [%s] %s --> %s", m.Type, filename, m.OutputDir)

	return nil
}

func (m *mrsOut) convertToMrs(ipRanges []netipx.IPRange, w io.Writer) (err error) {
	encoder, err := zstd.NewWriter(w)
	if err != nil {
		return err
	}
	defer encoder.Close()

	// header
	_, err = encoder.Write(mrsMagicBytes[:])
	if err != nil {
		return err
	}

	// behavior
	_, err = encoder.Write([]byte{1}) // RuleBehavior IPCIDR = 1
	if err != nil {
		return err
	}

	// count
	count := int64(len(ipRanges))
	err = binary.Write(encoder, binary.BigEndian, count)
	if err != nil {
		return err
	}

	// extra (reserved for future using)
	var extra []byte
	err = binary.Write(encoder, binary.BigEndian, int64(len(extra)))
	if err != nil {
		return err
	}
	_, err = encoder.Write(extra)
	if err != nil {
		return err
	}

	//
	// rule
	//
	// version
	_, err = encoder.Write([]byte{1})
	if err != nil {
		return err
	}

	// rule length
	err = binary.Write(encoder, binary.BigEndian, int64(len(ipRanges)))
	if err != nil {
		return err
	}

	for _, ipRange := range ipRanges {
		err = binary.Write(encoder, binary.BigEndian, ipRange.From().As16())
		if err != nil {
			return err
		}

		err = binary.Write(encoder, binary.BigEndian, ipRange.To().As16())
		if err != nil {
			return err
		}
	}

	return nil
}
