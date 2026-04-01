package cmd

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/jotx19/jotline/internal/ui"
	"github.com/spf13/cobra"
)

var listCmd = &cobra.Command{
	Use:   "list",
	Short: "List all ingested documents",
	RunE: func(cmd *cobra.Command, args []string) error {
		ui.Header("v0.1.0")

		cfg, err := config.Load()
		if err != nil {
			ui.Error(err.Error() + " — run: jotline init")
			return nil
		}

		s, err := store.New(cfg.MongoURI, cfg.Database)
		if err != nil {
			ui.Error("MongoDB connection failed: " + err.Error())
			return nil
		}

		docs, err := s.List()
		if err != nil {
			ui.Error(err.Error())
			return nil
		}

		if len(docs) == 0 {
			ui.Info("No documents yet — run: jotline ingest <file>")
			return nil
		}

		idStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#6B7280"))
		typeStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#7C3AED")).Bold(true)
		nameStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#F9FAFB")).Bold(true)

		fmt.Printf("\n  %d document(s)\n\n", len(docs))
		for _, d := range docs {
			fmt.Printf("  %s  %s  %s  %s\n",
				idStyle.Render(d.ID.Hex()[:8]+"..."),
				nameStyle.Render(fmt.Sprintf("%-28s", truncate(d.Title, 28))),
				typeStyle.Render(fmt.Sprintf("%-5s", strings.ToUpper(d.FileType))),
				idStyle.Render(fmt.Sprintf("%dk chars · %s",
					d.CharCount/1000,
					d.CreatedAt.Format("Jan 2, 2006"))),
			)
		}
		fmt.Println()
		return nil
	},
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-3] + "..."
}
