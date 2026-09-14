package utils

import (
	"crypto/ecdh"
	"encoding/base64"
	"encoding/binary"
	"encoding/pem"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestGenerateECHKeys(t *testing.T) {
	for _, tc := range []struct {
		name   string
		aeads  []string
		wantID []uint16
	}{
		{"defaults", nil, []uint16{1}},
		{"all", []string{"aes-128-gcm", "aes-256-gcm", "chacha20-poly1305"}, []uint16{1, 2, 3}},
		{"aes128", []string{"aes-128-gcm"}, []uint16{1}},
		{"aes256", []string{"aes-256-gcm"}, []uint16{2}},
		{"chacha", []string{"chacha20-poly1305"}, []uint16{3}},
		{"preference", []string{"chacha20-poly1305", "aes-128-gcm"}, []uint16{3, 1}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			id := uint8(255)
			data, list, err := GenerateECHKeys(ECHKeyOptions{
				PublicName: "public.example.com", ConfigID: &id, MaxNameLength: 255, AEADs: tc.aeads,
			})
			require.NoError(t, err)
			keyBlock, rest := pem.Decode(data)
			require.NotNil(t, keyBlock)
			require.Equal(t, pemBlockECHKeys, keyBlock.Type)
			publicBlock, rest := pem.Decode(rest)
			require.NotNil(t, publicBlock)
			require.Equal(t, pemBlockECHConfigs, publicBlock.Type)
			require.Empty(t, rest)
			require.Equal(t, list, publicBlock.Bytes)
			path := filepath.Join(t.TempDir(), "ech.pem")
			require.NoError(t, os.WriteFile(path, data, 0o600))
			keys, derived, err := LoadECHKeys(path)
			require.NoError(t, err)
			require.Len(t, keys, 1)
			require.Equal(t, list, derived)
			require.True(t, keys[0].SendAsRetry)
			for _, input := range []string{path, base64.StdEncoding.EncodeToString(list)} {
				parsed, err := ParseECHConfigList(input)
				require.NoError(t, err)
				require.Equal(t, list, parsed)
			}
			// Inspect the wire fields independently of the generator.
			config := keys[0].Config
			require.Equal(t, uint16(0xfe0d), binary.BigEndian.Uint16(config))
			require.Equal(t, len(config)-4, int(binary.BigEndian.Uint16(config[2:])))
			require.Equal(t, id, config[4])
			require.Equal(t, uint16(0x20), binary.BigEndian.Uint16(config[5:]))
			pub, remainder, err := readU16Prefixed(config[7:])
			require.NoError(t, err)
			privateKey, err := ecdh.X25519().NewPrivateKey(keys[0].PrivateKey)
			require.NoError(t, err)
			require.Equal(t, privateKey.PublicKey().Bytes(), pub)
			suites, remainder, err := readU16Prefixed(remainder)
			require.NoError(t, err)
			require.Len(t, suites, len(tc.wantID)*4)
			for i, aead := range tc.wantID {
				require.Equal(t, uint16(1), binary.BigEndian.Uint16(suites[i*4:]))
				require.Equal(t, aead, binary.BigEndian.Uint16(suites[i*4+2:]))
			}
			require.Equal(t, byte(255), remainder[0])
			require.Equal(t, "public.example.com", string(remainder[2:2+int(remainder[1])]))
			require.Equal(t, []byte{0, 0}, remainder[2+int(remainder[1]):])
			// Exercise Go TLS's real ECH decryption for each supported suite.
			assertECHHandshake(t, keys, list, true)
		})
	}
}

func TestGenerateECHKeysDefaultsAndFreshness(t *testing.T) {
	first, list, err := GenerateECHKeys(ECHKeyOptions{PublicName: "public.example.com"})
	require.NoError(t, err)
	second, otherList, err := GenerateECHKeys(ECHKeyOptions{PublicName: "public.example.com"})
	require.NoError(t, err)
	require.NotEqual(t, first, second)
	require.NotEqual(t, list, otherList)
	// Config IDs may legitimately collide; key material must not.
	block, _ := pem.Decode(first)
	keys, err := parseECHKeysBlob(block.Bytes)
	require.NoError(t, err)
	pub, rest, err := readU16Prefixed(keys[0].Config[7:])
	require.NoError(t, err)
	require.Len(t, pub, 32)
	_, rest, err = readU16Prefixed(rest)
	require.NoError(t, err)
	require.Zero(t, rest[0], "default maximum_name_length means unknown")
}

func TestGenerateECHKeysInvalidOptions(t *testing.T) {
	for _, name := range []string{
		"", "localhost", "127.0.0.1", "127.1", "example.123", "::1", "https://example.com", "example.com:443",
		"*.example.com", "example.com.", "a..com", "-a.com", "a-.com", "a_b.com",
		" example.com", "幽默老钟.com", strings.Repeat("a", 64) + ".com",
		strings.Repeat("a.", 127) + "com",
	} {
		t.Run(name, func(t *testing.T) {
			_, _, err := GenerateECHKeys(ECHKeyOptions{PublicName: name})
			require.Error(t, err)
		})
	}
	for _, aeads := range [][]string{{}, {"unknown"}, {"aes-128-gcm", "aes-128-gcm"}} {
		_, _, err := GenerateECHKeys(ECHKeyOptions{PublicName: "public.example.com", AEADs: aeads})
		require.Error(t, err)
	}
	_, _, err := GenerateECHKeys(ECHKeyOptions{PublicName: "xn--fsqu00a.example"})
	require.NoError(t, err)
}

func TestGenerateECHKeysMaximumPublicName(t *testing.T) {
	name := strings.Repeat(strings.Repeat("a", 63)+".", 3) + strings.Repeat("b", 61)
	require.Len(t, name, 253)
	data, list, err := GenerateECHKeys(ECHKeyOptions{PublicName: name})
	require.NoError(t, err)
	block, _ := pem.Decode(data)
	keys, err := parseECHKeysBlob(block.Bytes)
	require.NoError(t, err)
	assertECHHandshake(t, keys, list, true)
}
