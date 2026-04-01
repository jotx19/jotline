package chat

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jotx19/jotline/internal/store"
	"github.com/jotx19/jotline/internal/ui"
)

var (
	accent = lipgloss.Color("#7C3AED")
	green  = lipgloss.Color("#22C55E")
	dim    = lipgloss.Color("#6B7280")
	white  = lipgloss.Color("#F9FAFB")
	red    = lipgloss.Color("#EF4444")
	yellow = lipgloss.Color("#F59E0B")

	userBubble = lipgloss.NewStyle().
			Foreground(white).
			Background(accent).
			Padding(0, 1).
			Bold(true)

	assistantBubble = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(accent).
			Padding(0, 1).
			Width(60)

	toolStyle = lipgloss.NewStyle().
			Foreground(yellow).
			Italic(true)

	dimStyle = lipgloss.NewStyle().
			Foreground(dim)

	successStyle = lipgloss.NewStyle().
			Foreground(green).
			Bold(true)

	errorStyle = lipgloss.NewStyle().
			Foreground(red).
			Bold(true)

	titleStyle = lipgloss.NewStyle().
			Foreground(accent).
			Bold(true)

	promptStyle = lipgloss.NewStyle().
			Foreground(accent).
			Bold(true)

	modelBadge = lipgloss.NewStyle().
			Foreground(white).
			Background(accent).
			Padding(0, 1).
			Bold(true)
)

type Message struct {
	Role      string     `json:"role"`
	Content   string     `json:"content,omitempty"`
	ToolCalls []ToolCall `json:"tool_calls,omitempty"`
	Name      string     `json:"name,omitempty"`
}

type ToolCall struct {
	Function ToolCallFunction `json:"function"`
}

type ToolCallFunction struct {
	Name      string                 `json:"name"`
	Arguments map[string]interface{} `json:"arguments"`
}

type Tool struct {
	Type     string       `json:"type"`
	Function ToolFunction `json:"function"`
}

type ToolFunction struct {
	Name        string                 `json:"name"`
	Description string                 `json:"description"`
	Parameters  map[string]interface{} `json:"parameters"`
}

type ChatRequest struct {
	Model    string    `json:"model"`
	Messages []Message `json:"messages"`
	Tools    []Tool    `json:"tools,omitempty"`
	Stream   bool      `json:"stream"`
}

type ChatResponse struct {
	Message struct {
		Role      string     `json:"role"`
		Content   string     `json:"content"`
		ToolCalls []ToolCall `json:"tool_calls,omitempty"`
	} `json:"message"`
}

func jotlineTools() []Tool {
	return []Tool{
		{
			Type: "function",
			Function: ToolFunction{
				Name:        "list_documents",
				Description: "List all documents ingested into jotline",
				Parameters: map[string]interface{}{
					"type":       "object",
					"properties": map[string]interface{}{},
				},
			},
		},
		{
			Type: "function",
			Function: ToolFunction{
				Name:        "search_documents",
				Description: "Search across all ingested documents by keyword",
				Parameters: map[string]interface{}{
					"type": "object",
					"properties": map[string]interface{}{
						"query": map[string]interface{}{
							"type":        "string",
							"description": "Keyword or topic to search for",
						},
					},
					"required": []string{"query"},
				},
			},
		},
		{
			Type: "function",
			Function: ToolFunction{
				Name:        "get_document",
				Description: "Get full content of a document by its ID",
				Parameters: map[string]interface{}{
					"type": "object",
					"properties": map[string]interface{}{
						"id": map[string]interface{}{
							"type":        "string",
							"description": "Document ID",
						},
					},
					"required": []string{"id"},
				},
			},
		},
	}
}

func executeTool(s *store.Store, name string, args map[string]interface{}) string {
	switch name {
	case "list_documents":
		docs, err := s.List()
		if err != nil {
			return "Error: " + err.Error()
		}
		if len(docs) == 0 {
			return "No documents ingested yet."
		}
		var sb strings.Builder
		sb.WriteString(fmt.Sprintf("%d document(s):\n\n", len(docs)))
		for _, d := range docs {
			sb.WriteString(fmt.Sprintf("• [%s] %s (%s) — %d chars\n",
				d.ID.Hex(), d.Title, strings.ToUpper(d.FileType), d.CharCount))
		}
		return sb.String()

	case "search_documents":
		query, _ := args["query"].(string)
		if query == "" {
			return "query is required"
		}
		docs, err := s.Search(query)
		if err != nil {
			return "Error: " + err.Error()
		}
		if len(docs) == 0 {
			return fmt.Sprintf("No results for: %q", query)
		}
		var sb strings.Builder
		for _, d := range docs {
			sb.WriteString(fmt.Sprintf("### %s (%s)\n", d.Title, strings.ToUpper(d.FileType)))
			preview := d.RawText
			if len(preview) > 800 {
				preview = preview[:800] + "..."
			}
			sb.WriteString(preview + "\n\n---\n\n")
		}
		return sb.String()

	case "get_document":
		id, _ := args["id"].(string)
		if id == "" {
			return "id is required"
		}
		doc, err := s.GetByID(id)
		if err != nil {
			return "Error: " + err.Error()
		}
		return fmt.Sprintf("# %s\nType: %s | %d chars\n\n%s",
			doc.Title, strings.ToUpper(doc.FileType), doc.CharCount, doc.RawText)
	}
	return "unknown tool: " + name
}

func fetchModels() ([]string, error) {
	resp, err := http.Get("http://localhost:11434/api/tags")
	if err != nil {
		return nil, fmt.Errorf("cannot reach Ollama — is the Mac app running?")
	}
	defer resp.Body.Close()

	var result struct {
		Models []struct {
			Name string `json:"name"`
		} `json:"models"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, err
	}

	names := make([]string, len(result.Models))
	for i, m := range result.Models {
		names[i] = m.Name
	}
	return names, nil
}

func callOllama(model string, messages []Message, tools []Tool) (*ChatResponse, error) {
	payload := ChatRequest{
		Model:    model,
		Messages: messages,
		Tools:    tools,
		Stream:   false,
	}
	body, _ := json.Marshal(payload)

	resp, err := http.Post("http://localhost:11434/api/chat", "application/json", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("ollama error: %w", err)
	}
	defer resp.Body.Close()

	raw, _ := io.ReadAll(resp.Body)
	var chatResp ChatResponse
	if err := json.Unmarshal(raw, &chatResp); err != nil {
		return nil, fmt.Errorf("bad response: %s", string(raw))
	}
	return &chatResp, nil
}

type pickerModel struct {
	models   []string
	cursor   int
	selected string
}

func (m pickerModel) Init() tea.Cmd { return nil }

func (m pickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "up", "k":
			if m.cursor > 0 {
				m.cursor--
			}
		case "down", "j":
			if m.cursor < len(m.models)-1 {
				m.cursor++
			}
		case "enter":
			m.selected = m.models[m.cursor]
			return m, tea.Quit
		case "ctrl+c", "q":
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m pickerModel) View() string {
	var sb strings.Builder
	sb.WriteString("\n" + titleStyle.Render("◆ jotline") + dimStyle.Render("  select a model") + "\n\n")

	for i, name := range m.models {
		if i == m.cursor {
			sb.WriteString(successStyle.Render("  ❯ " + name))
		} else {
			sb.WriteString(dimStyle.Render("    " + name))
		}
		sb.WriteString("\n")
	}
	sb.WriteString("\n" + dimStyle.Render("  ↑/↓ move   enter select   q quit") + "\n")
	return sb.String()
}

func pickModel() (string, error) {
	models, err := fetchModels()
	if err != nil {
		return "", err
	}
	if len(models) == 0 {
		return "", fmt.Errorf("no models found — run: ollama pull qwen2.5:3b")
	}

	p := tea.NewProgram(pickerModel{models: models}, tea.WithOutput(os.Stderr))
	final, err := p.Run()
	if err != nil {
		return "", err
	}
	result := final.(pickerModel)
	if result.selected == "" {
		return "", fmt.Errorf("no model selected")
	}
	return result.selected, nil
}

type chatEntry struct {
	role    string
	content string
}

type chatModel struct {
	store     *store.Store
	model     string
	history   []Message
	tools     []Tool
	entries   []chatEntry
	input     textinput.Model
	spinner   spinner.Model
	thinking  bool
	toolLabel string
	err       string
}

type responseMsg struct {
	resp *ChatResponse
	err  error
}

func newChatModel(s *store.Store, mdl string) chatModel {
	ti := textinput.New()
	ti.Placeholder = "Ask anything about your documents..."
	ti.Focus()
	ti.CharLimit = 500
	ti.Width = 60
	ti.PromptStyle = promptStyle
	ti.Prompt = "You ❯ "

	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(accent)

	return chatModel{
		store:   s,
		model:   mdl,
		tools:   jotlineTools(),
		input:   ti,
		spinner: sp,
		history: []Message{
			{
				Role:    "system",
				Content: "You are a helpful assistant with access to the user's personal document library via jotline tools. Use the tools whenever the user asks about their documents, notes, or stored content. Be concise and clear.",
			},
		},
	}
}

func (m chatModel) Init() tea.Cmd {
	return textinput.Blink
}

func (m chatModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.KeyMsg:
		if m.thinking {
			return m, nil
		}
		switch msg.Type {
		case tea.KeyCtrlC, tea.KeyEsc:
			return m, tea.Quit
		case tea.KeyEnter:
			userText := strings.TrimSpace(m.input.Value())
			if userText == "" {
				return m, nil
			}
			if userText == "exit" || userText == "quit" {
				return m, tea.Quit
			}
			m.input.SetValue("")
			m.entries = append(m.entries, chatEntry{role: "user", content: userText})
			m.history = append(m.history, Message{Role: "user", Content: userText})
			m.thinking = true
			m.toolLabel = ""
			m.err = ""

			hist := m.history
			tools := m.tools
			mdl := m.model
			return m, tea.Batch(
				m.spinner.Tick,
				func() tea.Msg {
					resp, err := callOllama(mdl, hist, tools)
					return responseMsg{resp: resp, err: err}
				},
			)
		}

	case responseMsg:
		if msg.err != nil {
			m.thinking = false
			m.err = msg.err.Error()
			return m, nil
		}

		resp := msg.resp

		if len(resp.Message.ToolCalls) == 0 {
			m.thinking = false
			m.toolLabel = ""
			m.history = append(m.history, Message{Role: "assistant", Content: resp.Message.Content})
			m.entries = append(m.entries, chatEntry{role: "assistant", content: resp.Message.Content})
			return m, nil
		}
		tc := resp.Message.ToolCalls[0]
		m.history = append(m.history, Message{Role: "assistant", ToolCalls: resp.Message.ToolCalls})
		m.toolLabel = tc.Function.Name

		s := m.store
		mdl := m.model
		hist := m.history
		name := tc.Function.Name
		args := tc.Function.Arguments
		return m, tea.Batch(
			m.spinner.Tick,
			func() tea.Msg {
				result := executeTool(s, name, args)
				newHist := append(hist, Message{Role: "tool", Name: name, Content: result})
				newHist = append(newHist, Message{Role: "user", Content: "Summarise the tool result above in plain, friendly language."})
				resp2, err := callOllama(mdl, newHist, nil)
				return responseMsg{resp: resp2, err: err}
			},
		)

	case spinner.TickMsg:
		if m.thinking {
			var cmd tea.Cmd
			m.spinner, cmd = m.spinner.Update(msg)
			return m, cmd
		}
	}

	if !m.thinking {
		var cmd tea.Cmd
		m.input, cmd = m.input.Update(msg)
		return m, cmd
	}

	return m, nil
}

func (m chatModel) View() string {
	var sb strings.Builder

	sb.WriteString("\n" + titleStyle.Render("◆ jotline") + "  " + modelBadge.Render(" "+m.model+" ") + "\n")
	sb.WriteString(dimStyle.Render("  ctrl+c to exit  •  your documents are available as context") + "\n\n")
	for _, e := range m.entries {
		switch e.role {
		case "user":
			sb.WriteString("  " + userBubble.Render(" You ") + "  " + e.content + "\n\n")
		case "assistant":
			sb.WriteString(assistantBubble.Render(e.content) + "\n\n")
		}
	}

	if m.thinking {
		label := "thinking..."
		if m.toolLabel != "" {
			label = "calling " + m.toolLabel + "..."
		}
		sb.WriteString("  " + m.spinner.View() + " " + dimStyle.Render(label) + "\n\n")
	}

	if m.err != "" {
		sb.WriteString("  " + errorStyle.Render("✗ "+m.err) + "\n\n")
	}
	sb.WriteString("  " + m.input.View() + "\n")

	return sb.String()
}

func Start(s *store.Store) error {
	ui.Header("v0.1.0")

	model, err := pickModel()
	if err != nil {
		ui.Error(err.Error())
		return err
	}

	ui.Success("Connected to " + model)
	fmt.Fprintln(os.Stderr)

	p := tea.NewProgram(newChatModel(s, model), tea.WithAltScreen())
	_, err = p.Run()
	return err
}
