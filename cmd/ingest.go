package cmd

import (
	"fmt"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/extractor"
	"github.com/jotx19/jotline/internal/store"
	"github.com/jotx19/jotline/internal/ui"
	"github.com/spf13/cobra"
)

var ingestCmd = &cobra.Command{
	Use:     "ingest <file>",
	Short:   "Ingest a PDF, DOCX or text file",
	Args:    cobra.ExactArgs(1),
	Example: `  jotline ingest ./lecture1.pdf`,
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		ui.Header("v0.1.0")

		cfg, err := config.Load()
		if err != nil {
			ui.Error(err.Error() + " — run: jotline init")
			return nil
		}

		result, err := ui.RunWithSpinner(
			"Extracting "+filePath,
			func() (string, error) {
				r, err := extractor.Extract(filePath)
				if err != nil {
					return "", err
				}
				s, err := store.New(cfg.MongoURI, cfg.Database)
				if err != nil {
					return "", err
				}
				doc := &store.Document{
					Title:    r.Title,
					FileName: r.FileName,
					FileType: r.FileType,
					RawText:  r.RawText,
					Size:     r.Size,
				}
				if err := s.Save(doc); err != nil {
					return "", err
				}
				return fmt.Sprintf("%s · %d chars · ID: %s",
					r.Title, len(r.RawText), doc.ID.Hex()), nil
			},
		)

		if err != nil {
			ui.Error(err.Error())
			return nil
		}

		ui.Success("Ingested: " + result)
		return nil
	},
}
