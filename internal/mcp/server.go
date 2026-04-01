package mcp

import (
	"context"
	"fmt"
	"strings"

	"github.com/jotx19/jotline/internal/store"
	"github.com/mark3labs/mcp-go/mcp"
	"github.com/mark3labs/mcp-go/server"
)

func getArgs(req mcp.CallToolRequest) (map[string]interface{}, error) {
	args, ok := req.Params.Arguments.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("invalid arguments format")
	}
	return args, nil
}

func Serve(s *store.Store) error {
	srv := server.NewMCPServer(
		"jotline",
		"1.0.0",
		server.WithToolCapabilities(true),
	)

	srv.AddTool(
		mcp.NewTool("list_documents",
			mcp.WithDescription("List all documents ingested into jotline"),
		),
		func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			docs, err := s.List()
			if err != nil {
				return mcp.NewToolResultText("Error: " + err.Error()), nil
			}

			if len(docs) == 0 {
				return mcp.NewToolResultText("No documents yet."), nil
			}

			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("%d document(s):\n\n", len(docs)))

			for _, d := range docs {
				sb.WriteString(fmt.Sprintf(
					"• [%s] %s (%s) — %d chars\n",
					d.ID.Hex(),
					d.Title,
					strings.ToUpper(d.FileType),
					d.CharCount,
				))
			}

			return mcp.NewToolResultText(sb.String()), nil
		},
	)

	srv.AddTool(
		mcp.NewTool("search_documents",
			mcp.WithDescription("Search across all ingested documents by keyword"),
			mcp.WithString("query",
				mcp.Required(),
				mcp.Description("Keyword or topic to search for"),
			),
		),
		func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {

			args, err := getArgs(req)
			if err != nil {
				return mcp.NewToolResultText(err.Error()), nil
			}

			query, ok := args["query"].(string)
			if !ok || query == "" {
				return mcp.NewToolResultText("query is required"), nil
			}

			docs, err := s.Search(query)
			if err != nil {
				return mcp.NewToolResultText("Error: " + err.Error()), nil
			}

			if len(docs) == 0 {
				return mcp.NewToolResultText(fmt.Sprintf("No results for: %q", query)), nil
			}

			var sb strings.Builder

			for _, d := range docs {
				sb.WriteString(fmt.Sprintf(
					"### %s (%s)\n",
					d.Title,
					strings.ToUpper(d.FileType),
				))

				preview := d.RawText
				if len(preview) > 800 {
					preview = preview[:800] + "..."
				}

				sb.WriteString(preview)
				sb.WriteString("\n\n---\n\n")
			}

			return mcp.NewToolResultText(sb.String()), nil
		},
	)

	srv.AddTool(
		mcp.NewTool("get_document",
			mcp.WithDescription("Get the full content of a document by its ID"),
			mcp.WithString("id",
				mcp.Required(),
				mcp.Description("Document ID from list_documents or search_documents"),
			),
		),
		func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {

			args, err := getArgs(req)
			if err != nil {
				return mcp.NewToolResultText(err.Error()), nil
			}

			id, ok := args["id"].(string)
			if !ok || id == "" {
				return mcp.NewToolResultText("id is required"), nil
			}

			doc, err := s.GetByID(id)
			if err != nil {
				return mcp.NewToolResultText("Error: " + err.Error()), nil
			}

			text := fmt.Sprintf(
				"# %s\nType: %s | %d chars\nIngested: %s\n\n%s",
				doc.Title,
				strings.ToUpper(doc.FileType),
				doc.CharCount,
				doc.CreatedAt.Format("Jan 2, 2006 15:04"),
				doc.RawText,
			)

			return mcp.NewToolResultText(text), nil
		},
	)

	return server.ServeStdio(srv)
}
