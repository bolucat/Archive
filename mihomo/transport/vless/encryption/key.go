package encryption

import (
	"crypto/rand"
	"encoding/base64"
	"fmt"

	"github.com/metacubex/utls/mlkem"
)

func GenMLKEM768(seedStr string) (seedBase64, pubBase64 string, err error) {
	var seed [64]byte
	if len(seedStr) > 0 {
		s, _ := base64.RawURLEncoding.DecodeString(seedStr)
		if len(s) != 64 {
			err = fmt.Errorf("invalid length of ML-KEM-768 seed: %s", seedStr)
			return
		}
		seed = [64]byte(s)
	} else {
		_, err = rand.Read(seed[:])
		if err != nil {
			return
		}
	}

	key, _ := mlkem.NewDecapsulationKey768(seed[:])
	pub := key.EncapsulationKey()
	seedBase64 = base64.RawURLEncoding.EncodeToString(seed[:])
	pubBase64 = base64.RawURLEncoding.EncodeToString(pub.Bytes())
	return
}
