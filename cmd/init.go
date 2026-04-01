package cmd

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/spf13/cobra"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Set up pipeline (run this first)",
	RunE: func(cmd *cobra.Command, args []string) error {
		fmt.Println("\n🚀 Welcome to Pipeline")
		fmt.Println("Feed your documents to ai via MCP.\n")

		// check if already configured
		if cfg, err := config.Load(); err == nil && cfg.MongoURI != "" {
			fmt.Println("Already configured.")
			fmt.Print("Reconfigure? [y/N]: ")
			reader := bufio.NewReader(os.Stdin)
			answer, _ := reader.ReadString('\n')
			if !strings.HasPrefix(strings.ToLower(strings.TrimSpace(answer)), "y") {
				fmt.Println("Keeping existing config.")
				return nil
			}
		}

		reader := bufio.NewReader(os.Stdin)
		fmt.Print("Enter your MongoDB URI: ")
		uri, _ := reader.ReadString('\n')
		uri = strings.TrimSpace(uri)

		if uri == "" {
			return fmt.Errorf("MongoDB URI cannot be empty")
		}

		fmt.Print("Testing connection...")
		if err := store.TestConnection(uri); err != nil {
			fmt.Println(" ❌")
			return fmt.Errorf("connection failed: %w", err)
		}
		fmt.Println(" ✅")

		if err := config.Save(&config.Config{MongoURI: uri}); err != nil {
			return fmt.Errorf("failed to save config: %w", err)
		}

		fmt.Println("\n✅ Pipeline ready!")
		fmt.Println("\nNext:")
		fmt.Println("  pipeline ingest ./your-notes.pdf")
		fmt.Println("  pipeline list")
		fmt.Println("  pipeline serve")
		return nil
	},
}
