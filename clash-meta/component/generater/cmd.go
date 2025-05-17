package generater

import (
	"encoding/base64"
	"fmt"

	"github.com/metacubex/mihomo/component/ech"

	"github.com/gofrs/uuid/v5"
)

func Main(args []string) {
	if len(args) < 1 {
		panic("Using: generate uuid/reality-keypair/wg-keypair/ech-keypair")
	}
	switch args[0] {
	case "uuid":
		newUUID, err := uuid.NewV4()
		if err != nil {
			panic(err)
		}
		fmt.Println(newUUID.String())
	case "reality-keypair":
		privateKey, err := GeneratePrivateKey()
		if err != nil {
			panic(err)
		}
		publicKey := privateKey.PublicKey()
		fmt.Println("PrivateKey: " + base64.RawURLEncoding.EncodeToString(privateKey[:]))
		fmt.Println("PublicKey: " + base64.RawURLEncoding.EncodeToString(publicKey[:]))
	case "wg-keypair":
		privateKey, err := GeneratePrivateKey()
		if err != nil {
			panic(err)
		}
		fmt.Println("PrivateKey: " + privateKey.String())
		fmt.Println("PublicKey: " + privateKey.PublicKey().String())
	case "ech-keypair":
		if len(args) < 2 {
			panic("Using: generate ech-keypair <plain_server_name>")
		}
		configBase64, keyPem, err := ech.GenECHConfig(args[1])
		if err != nil {
			panic(err)
		}
		fmt.Println("Config:", configBase64)
		fmt.Println("Key:", keyPem)
	}
}
