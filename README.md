# Jotline (Pipeline) 🚧 In Development
> **This project is currently under active development and is not ready for production use.**


A CLI tool to feed your documents to Claude (or any AI agent) via MCP.  
Ingest your PDFs, DOCX, and text files from the terminal — Claude reads everything.

---

## What it does

You run:  pipeline ingest ./notes.pdf
Claude knows everything in those files via MCP.

No web UI. No uploads. Just your terminal and your files.

---

## MVP Features

- `pipeline init` — connect your MongoDB, one time setup
- `pipeline ingest <file>` — ingest PDF, DOCX, TXT, or MD
- `pipeline list` — see everything you've ingested
- `pipeline delete <id>` — remove a document
- `pipeline serve` — start MCP server for Claude Desktop

---

## Installation (coming soon)

```bash
brew install jotx19/tap/pipeline
```

For now, build from source:

```bash
git clone https://github.com/jotx19/pipeline
cd pipeline
go build -o pipeline .
./pipeline init
```

---

## Quick Start

```bash
# 1. Set up (enter your MongoDB URI when prompted)
pipeline init

# 2. Ingest your files
pipeline ingest ./notes.pdf
pipeline ingest ./vocab.txt

# 3. Check what's stored
pipeline list

# 4. Start MCP server
pipeline serve
```

---

## Connect to Claude Desktop

Add this to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "pipeline": {
      "command": "pipeline",
      "args": ["serve"]
    }
  }
}
```

Restart Claude Desktop. You now have three tools:

- `list_documents` — see everything ingested
- `search_documents` — search by keyword
- `get_document` — read full content of any file

---

## Tech Stack

- **Go** — single static binary, no runtime deps
- **Cobra** — CLI framework
- **MongoDB** — your own Atlas cluster, your data
- **mark3labs/mcp-go** — MCP server
- **ledongthuc/pdf** — PDF extraction
- **lu4p/cat** — DOCX extraction

---

## Roadmap

- [x] PDF, DOCX, TXT, MD extraction
- [x] MongoDB storage
- [x] MCP server with list, search, get tools
- [ ] Homebrew distribution
- [ ] `pipeline search <query>` CLI command
- [ ] Hosted MCP server with API key auth
- [ ] Multiple file ingest at once (`pipeline ingest ./folder/`)

---

## Requirements

- Go 1.22+
- MongoDB Atlas account (free tier works)

---

## Status

🚧 **Under development** — core ingestion and MCP server are working.  
Homebrew distribution and hosted server coming soon.

---

## Author

Built by [@jotx19](https://github.com/jotx19)