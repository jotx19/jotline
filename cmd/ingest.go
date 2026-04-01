package cmd

import (
	"fmt"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/extractor"
	"github.com/jotx19/jotline/internal/store"
	"github.com/spf13/cobra"
)

var ingestCmd = &cobra.Command{
	Use:   "ingest <file>",
	Short: "Ingest a PDF, DOCX or text file",
	Args:  cobra.ExactArgs(1),
	Example: `  pipeline ingest ./lecture1.pdf
  pipeline ingest ./vocab.txt
  pipeline ingest ./notes.md`,
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		cfg, err := config.Load()
		if err != nil {
			return fmt.Errorf("%s — run: pipeline init", err.Error())
		}

		fmt.Printf("Extracting %s...\n", filePath)
		result, err := extractor.Extract(filePath)
		if err != nil {
			return err
		}

		s, err := store.New(cfg.MongoURI, cfg.Database)
		if err != nil {
			return fmt.Errorf("failed to connect to MongoDB: %w", err)
		}

		doc := &store.Document{
			Title:    result.Title,
			FileName: result.FileName,
			FileType: result.FileType,
			RawText:  result.RawText,
			Size:     result.Size,
		}

		if err := s.Save(doc); err != nil {
			return fmt.Errorf("failed to save: %w", err)
		}

		fmt.Printf("✅ Ingested: %s\n", result.Title)
		fmt.Printf("   %d chars | ID: %s\n", len(result.RawText), doc.ID.Hex())
		return nil
	},
}
