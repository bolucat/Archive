package cmd

import (
	"bytes"
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/apernet/hysteria/app/v2/internal/utils"
	"github.com/stretchr/testify/require"
)

func executeECH(args ...string) (string, error) {
	cmd := newECHCmd()
	var out bytes.Buffer
	cmd.SetOut(&out)
	cmd.SetErr(&out)
	cmd.SetArgs(args)
	err := cmd.Execute()
	return out.String(), err
}

func TestECHCommand(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ech key.pem")
	out, err := executeECH("--public-name", "public.example.com", "--output", path,
		"--config-id", "0", "--max-name-length", "64", "--aead", "chacha20-poly1305,aes-128-gcm")
	require.NoError(t, err)
	keys, list, err := utils.LoadECHKeys(path)
	require.NoError(t, err)
	require.Zero(t, keys[0].Config[4])
	require.Contains(t, out, fmt.Sprintf("  keyPath: %q", path))
	require.Contains(t, out, "  ech: "+base64.StdEncoding.EncodeToString(list))
	require.NotContains(t, out, "BEGIN ECH KEYS")
	require.NotContains(t, out, base64.StdEncoding.EncodeToString(keys[0].PrivateKey))
	info, err := os.Stat(path)
	require.NoError(t, err)
	if runtime.GOOS != "windows" {
		require.Equal(t, os.FileMode(0o600), info.Mode().Perm())
	}
	before, err := os.ReadFile(path)
	require.NoError(t, err)
	_, err = executeECH("--public-name", "public.example.com", "--output", path)
	require.ErrorContains(t, err, "already exists")
	after, err := os.ReadFile(path)
	require.NoError(t, err)
	require.Equal(t, before, after)
	require.NoError(t, os.Chmod(path, 0o644))
	_, err = executeECH("--public-name", "public.example.com", "--output", path, "--overwrite")
	require.NoError(t, err)
	after, err = os.ReadFile(path)
	require.NoError(t, err)
	require.NotEqual(t, before, after)
	_, _, err = utils.LoadECHKeys(path)
	require.NoError(t, err)
	info, err = os.Stat(path)
	require.NoError(t, err)
	if runtime.GOOS != "windows" {
		require.Equal(t, os.FileMode(0o600), info.Mode().Perm())
	}
}

func TestECHCommandOutputErrors(t *testing.T) {
	for _, path := range []string{"", t.TempDir(), filepath.Join(t.TempDir(), "missing", "ech.pem")} {
		_, err := executeECH("--public-name", "public.example.com", "--output", path)
		require.Error(t, err)
	}
}

func TestECHCommandRequiresPublicNameAndValidFlags(t *testing.T) {
	for _, args := range [][]string{
		{},
		{"--public-name", ""},
		{"--public-name", "127.0.0.1"},
		{"--public-name", "public.example.com", "unexpected"},
		{"--public-name", "public.example.com", "--config-id", "256"},
		{"--public-name", "public.example.com", "--config-id", "-2"},
		{"--public-name", "public.example.com", "--max-name-length", "256"},
		{"--public-name", "public.example.com", "--max-name-length", "-1"},
		{"--public-name", "public.example.com", "--aead", ""},
		{"--public-name", "public.example.com", "--aead", "invalid"},
	} {
		path := filepath.Join(t.TempDir(), "ech.pem")
		_, err := executeECH(append(args, "--output", path)...)
		require.Error(t, err, "%v", args)
		_, err = os.Stat(path)
		require.True(t, os.IsNotExist(err), "invalid options must not create a key file")
	}
}
