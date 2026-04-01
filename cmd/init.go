package cmd

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/jotx19/jotline/internal/ui"
	"github.com/spf13/cobra"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Set up jotline (run this first)",
	RunE: func(cmd *cobra.Command, args []string) error {
		ui.Header("v0.1.0")

		if cfg, err := config.Load(); err == nil && cfg.MongoURI != "" {
			ui.Info("Already configured.")
			fmt.Print("  Reconfigure? [y/N]: ")
			reader := bufio.NewReader(os.Stdin)
			answer, _ := reader.ReadString('\n')
			if !strings.HasPrefix(strings.ToLower(strings.TrimSpace(answer)), "y") {
				ui.Info("Keeping existing config.")
				return nil
			}
		}

		reader := bufio.NewReader(os.Stdin)
		fmt.Print("  Enter MongoDB URI: ")
		uri, _ := reader.ReadString('\n')
		uri = strings.TrimSpace(uri)

		if uri == "" {
			ui.Error("MongoDB URI cannot be empty")
			return nil
		}

		_, err := ui.RunWithSpinner("Testing connection", func() (string, error) {
			return "", store.TestConnection(uri)
		})

		if err != nil {
			ui.Error("Connection failed: " + err.Error())
			return nil
		}

		config.Save(&config.Config{MongoURI: uri})

		ui.Success("Pipeline ready!")
		ui.Info("jotline ingest ./your-notes.pdf")
		ui.Info("jotline list")
		ui.Info("jotline serve")
		return nil
	},
}
