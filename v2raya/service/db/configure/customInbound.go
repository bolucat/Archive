package configure

import "github.com/v2rayA/v2rayA/db"

// CustomInbound represents a user-defined inbound proxy port.
// Protocol must be "socks" or "http".
// Tag must be unique and is used as the inbound tag in v2ray config.
type CustomInbound struct {
	Tag      string `json:"tag"`
	Protocol string `json:"protocol"` // "socks" or "http"
	Port     int    `json:"port"`
}

// GetCustomInbounds returns all custom inbound configs stored in DB.
func GetCustomInbounds() []CustomInbound {
	var result []CustomInbound
	_ = db.Get("system", "customInbounds", &result)
	if result == nil {
		result = []CustomInbound{}
	}
	return result
}

// SetCustomInbounds persists the custom inbound list to DB.
func SetCustomInbounds(inbounds []CustomInbound) error {
	return db.Set("system", "customInbounds", inbounds)
}
