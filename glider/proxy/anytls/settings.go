package anytls

import (
	"fmt"
	"strconv"
	"strings"
)

func encodeSettings(m map[string]string) []byte {
	keys := []string{"v", "client", "padding-md5"}
	lines := make([]string, 0, len(m))
	seen := map[string]bool{}
	for _, k := range keys {
		if v, ok := m[k]; ok {
			lines = append(lines, k+"="+v)
			seen[k] = true
		}
	}
	for k, v := range m {
		if !seen[k] {
			lines = append(lines, k+"="+v)
		}
	}
	return []byte(strings.Join(lines, "\n"))
}

func parseSettings(data []byte) map[string]string {
	out := map[string]string{}
	for line := range strings.SplitSeq(string(data), "\n") {
		if line == "" {
			continue
		}
		k, v, ok := strings.Cut(line, "=")
		if ok {
			out[k] = v
		}
	}
	return out
}

func settingsVersion(m map[string]string) int {
	v, err := strconv.Atoi(m["v"])
	if err != nil {
		return 1
	}
	return v
}

func clientSettings(ps paddingScheme) []byte {
	return encodeSettings(map[string]string{
		"v":           fmt.Sprint(protocolVersion),
		"client":      "glider-anytls",
		"padding-md5": ps.md5(),
	})
}

func serverSettings() []byte {
	return encodeSettings(map[string]string{"v": fmt.Sprint(protocolVersion)})
}
