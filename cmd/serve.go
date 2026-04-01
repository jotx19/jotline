package cmd

import (
	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/mcp"
	"github.com/jotx19/jotline/internal/store"
	"github.com/jotx19/jotline/internal/ui"
	"github.com/spf13/cobra"
)

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Start the MCP server for Claude Desktop",
	RunE: func(cmd *cobra.Command, args []string) error {
		ui.Header("v0.1.0")

		cfg, err := config.Load()
		if err != nil {
			ui.Error(err.Error() + " — run: jotline init")
			return nil
		}

		var s *store.Store
		_, err = ui.RunWithSpinner("Connecting to MongoDB", func() (string, error) {
			var err error
			s, err = store.New(cfg.MongoURI, cfg.Database)
			return "", err
		})

		if err != nil {
			ui.Error("MongoDB connection failed: " + err.Error())
			return nil
		}

		ui.Success("Database connected")
		ui.Info("MCP server starting...")

		ui.Info("Tools available:")
		ui.Info("  • List Documents")
		ui.Info("  • Search Documents")
		ui.Info("  • Get Documents")

		ui.Info("Waiting for client connection...")

		return mcp.Serve(s)
	},
}
