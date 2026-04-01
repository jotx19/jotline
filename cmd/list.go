package cmd

import (
	"fmt"
	"strings"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/spf13/cobra"
)

var listCmd = &cobra.Command{
	Use:   "list",
	Short: "List all ingested documents",
	RunE: func(cmd *cobra.Command, args []string) error {
		cfg, err := config.Load()
		if err != nil {
			return fmt.Errorf("%s — run: pipeline init", err.Error())
		}

		s, err := store.New(cfg.MongoURI, cfg.Database)
		if err != nil {
			return fmt.Errorf("failed to connect: %w", err)
		}

		docs, err := s.List()
		if err != nil {
			return err
		}

		if len(docs) == 0 {
			fmt.Println("No documents ingested yet.")
			fmt.Println("Run: pipeline ingest <file>")
			return nil
		}

		fmt.Printf("\n%d document(s):\n\n", len(docs))
		for _, d := range docs {
			fmt.Printf("  %s  %-30s  %-5s  %dk chars  %s\n",
				d.ID.Hex()[:8]+"...",
				truncate(d.Title, 30),
				strings.ToUpper(d.FileType),
				d.CharCount/1000,
				d.CreatedAt.Format("Jan 2, 2006"),
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
