package cmd

import (
	"fmt"

	"github.com/spf13/cobra"
)

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Start the MCP server for Claude Desktop",
	RunE: func(cmd *cobra.Command, args []string) error {
		fmt.Println("MCP server coming soon...")
		return nil
	},
}
