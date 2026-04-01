package cmd

import (
	"fmt"
	"os"

	"github.com/jotx19/jotline/internal/chat"
	"github.com/jotx19/jotline/internal/config"
	"github.com/jotx19/jotline/internal/store"
	"github.com/spf13/cobra"
)

var chatCmd = &cobra.Command{
	Use:   "chat",
	Short: "Chat with your documents using a local Ollama model",
	Run: func(cmd *cobra.Command, args []string) {
		cfg, err := config.Load()
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error loading config:", err)
			os.Exit(1)
		}

		s, err := store.New(cfg.MongoURI, cfg.Database)
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error connecting to database:", err)
			os.Exit(1)
		}

		if err := chat.Start(s); err != nil {
			fmt.Fprintln(os.Stderr, "Chat error:", err)
			os.Exit(1)
		}
	},
}

func init() {
	rootCmd.AddCommand(chatCmd)
}
