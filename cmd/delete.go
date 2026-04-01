package cmd

import (
	"fmt"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/spf13/cobra"
)

var deleteCmd = &cobra.Command{
	Use:     "delete <id>",
	Short:   "Delete a document",
	Args:    cobra.ExactArgs(1),
	Example: `  pipeline delete 507f1f77bcf86cd799439011`,
	RunE: func(cmd *cobra.Command, args []string) error {
		cfg, err := config.Load()
		if err != nil {
			return fmt.Errorf("%s — run: pipeline init", err.Error())
		}

		s, err := store.New(cfg.MongoURI, cfg.Database)
		if err != nil {
			return fmt.Errorf("failed to connect: %w", err)
		}

		if err := s.Delete(args[0]); err != nil {
			return err
		}

		fmt.Printf("✅ Deleted: %s\n", args[0])
		return nil
	},
}
