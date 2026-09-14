package cmd

import (
	"encoding/base64"
	"errors"
	"fmt"

	"github.com/apernet/hysteria/app/v2/internal/utils"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(newECHCmd())
}

func newECHCmd() *cobra.Command {
	var options utils.ECHKeyOptions
	var output string
	var configID int
	var overwrite bool
	cmd := &cobra.Command{
		Use:   "ech --public-name <name>",
		Short: "Generate ECH keys and client configuration",
		Long: "Generate an X25519/HKDF-SHA256 ECH key pair and print sample server and client configuration. " +
			"The public name is the plaintext outer SNI visible on the network; it must be supplied explicitly.",
		Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			if configID < -1 || configID > 255 {
				return errors.New("config-id must be between 0 and 255, or -1 for a random ID")
			}
			if output == "" {
				return errors.New("output path is empty")
			}
			options.ConfigID = nil
			if configID >= 0 {
				id := uint8(configID)
				options.ConfigID = &id
			}
			keyPEM, configList, err := utils.GenerateECHKeys(options)
			if err != nil {
				return err
			}
			if err := writeCertFile(output, keyPEM, 0o600, overwrite); err != nil {
				return fmt.Errorf("write ECH keys: %w", err)
			}
			_, err = fmt.Fprintf(cmd.OutOrStdout(),
				"Generated ECH key file: %s\nKeep this file private; share only the client config below.\n\n"+
					"# server.yaml\nech:\n  keyPath: %q\n\n"+
					"# client.yaml (keep your existing TLS certificate verification settings)\ntls:\n  ech: %s\n",
				output, output, base64.StdEncoding.EncodeToString(configList))
			return err
		},
	}
	cmd.Flags().StringVar(&options.PublicName, "public-name", "", "required plaintext outer SNI (DNS name)")
	cmd.Flags().IntVar(&configID, "config-id", -1, "ECH config ID, 0-255 (-1 selects a random ID)")
	cmd.Flags().Uint8Var(&options.MaxNameLength, "max-name-length", 0, "inner server name length padding hint, 0-255 (0 means unknown)")
	cmd.Flags().StringSliceVar(&options.AEADs, "aead", []string{"aes-128-gcm"}, "comma-separated HPKE AEADs: aes-128-gcm, aes-256-gcm, chacha20-poly1305 (in preference order)")
	cmd.Flags().StringVarP(&output, "output", "o", "ech.pem", "output ECH key file (private, mode 0600)")
	cmd.Flags().BoolVar(&overwrite, "overwrite", false, "overwrite an existing ECH key file")
	if err := cmd.MarkFlagRequired("public-name"); err != nil {
		panic(err)
	}
	return cmd
}
