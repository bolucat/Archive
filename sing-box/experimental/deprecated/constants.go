package deprecated

import (
	C "github.com/sagernet/sing-box/constant"
	F "github.com/sagernet/sing/common/format"

	"golang.org/x/mod/semver"
)

type Note struct {
	Name              string
	Description       string
	DeprecatedVersion string
	ScheduledVersion  string
	EnvName           string
	MigrationLink     string
}

func (n Note) Impending() bool {
	if n.ScheduledVersion == "" {
		return false
	}
	if !semver.IsValid("v" + C.Version) {
		return false
	}
	versionMinor := semver.Compare(semver.MajorMinor("v"+C.Version), "v"+n.ScheduledVersion)
	if versionMinor < 0 {
		panic("invalid deprecated note: " + n.Name)
	}
	return versionMinor <= 1
}

func (n Note) String() string {
	return F.ToString(
		n.Description, " is deprecated in sing-box ", n.DeprecatedVersion,
		" and will be removed in sing-box ", n.ScheduledVersion, ", checkout documentation for migration: ", n.MigrationLink,
	)
}

var OptionBadMatchSource = Note{
	Name:              "bad-match-source",
	Description:       "legacy match source rule item",
	DeprecatedVersion: "1.10.0",
	ScheduledVersion:  "1.11.0",
	MigrationLink:     "https://sing-box.sagernet.org/deprecated/#match-source-rule-items-are-renamed",
}

var OptionGEOIP = Note{
	Name:              "geoip",
	Description:       "geoip database",
	DeprecatedVersion: "1.8.0",
	ScheduledVersion:  "1.12.0",
	EnvName:           "GEOIP",
	MigrationLink:     "https://sing-box.sagernet.org/migration/#migrate-geoip-to-rule-sets",
}

var OptionGEOSITE = Note{
	Name:              "geosite",
	Description:       "geosite database",
	DeprecatedVersion: "1.8.0",
	ScheduledVersion:  "1.12.0",
	EnvName:           "GEOSITE",
	MigrationLink:     "https://sing-box.sagernet.org/migration/#migrate-geosite-to-rule-sets",
}

var OptionTUNAddressX = Note{
	Name:              "tun-address-x",
	Description:       "legacy tun address fields",
	DeprecatedVersion: "1.10.0",
	ScheduledVersion:  "1.12.0",
	MigrationLink:     "https://sing-box.sagernet.org/migration/#tun-address-fields-are-merged",
}

var Options = []Note{
	OptionBadMatchSource,
	OptionGEOIP,
	OptionGEOSITE,
	OptionTUNAddressX,
}
