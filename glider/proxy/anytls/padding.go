package anytls

import (
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"strconv"
	"strings"
)

const defaultPaddingScheme = "stop=8\n0=30-30\n1=100-400\n2=400-500,c,500-1000,c,500-1000,c,500-1000,c,500-1000\n3=9-9,500-1000\n4=500-1000\n5=500-1000\n6=500-1000\n7=500-1000"

type paddingScheme struct {
	raw       string
	authRange [2]int
}

func parsePaddingScheme(raw string) (paddingScheme, error) {
	if strings.TrimSpace(raw) == "" {
		raw = defaultPaddingScheme
	}
	ps := paddingScheme{raw: raw, authRange: [2]int{0, 0}}
	for line := range strings.SplitSeq(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			return ps, fmt.Errorf("invalid padding scheme line %q", line)
		}
		if key == "0" {
			r, err := parseRange(value)
			if err != nil {
				return ps, err
			}
			ps.authRange = r
		}
	}
	return ps, nil
}

func parseRange(s string) ([2]int, error) {
	part := strings.SplitN(s, ",", 2)[0]
	lo, hi, ok := strings.Cut(part, "-")
	if !ok {
		return [2]int{}, fmt.Errorf("invalid range %q", s)
	}
	min, err := strconv.Atoi(lo)
	if err != nil {
		return [2]int{}, err
	}
	max, err := strconv.Atoi(hi)
	if err != nil {
		return [2]int{}, err
	}
	if min < 0 || max < min || max > 65535 {
		return [2]int{}, fmt.Errorf("invalid range %q", s)
	}
	return [2]int{min, max}, nil
}

func (p paddingScheme) authPaddingLen() int {
	return p.authRange[0]
}

func (p paddingScheme) md5() string {
	sum := md5.Sum([]byte(p.raw))
	return hex.EncodeToString(sum[:])
}
