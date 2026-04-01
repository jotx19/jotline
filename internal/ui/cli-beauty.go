package ui

import (
	"fmt"
	"os"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

var (
	accent = lipgloss.Color("#7C3AED")
	green  = lipgloss.Color("#22C55E")
	dim    = lipgloss.Color("#6B7280")
	white  = lipgloss.Color("#F9FAFB")

	titleStyle = lipgloss.NewStyle().
			Foreground(accent).
			Bold(true)

	boxStyle = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(accent).
			Padding(0, 1).
			Width(45)

	successStyle = lipgloss.NewStyle().
			Foreground(green).
			Bold(true)

	dimStyle = lipgloss.NewStyle().
			Foreground(dim)

	labelStyle = lipgloss.NewStyle().
			Foreground(white).
			Bold(true)
)

func Header(version string) {
	fmt.Fprintln(os.Stderr)
	fmt.Fprintln(os.Stderr, titleStyle.Render("◆ jotline")+dimStyle.Render("  "+version))
	fmt.Fprintln(os.Stderr)
}

func Success(msg string) {
	fmt.Fprintln(os.Stderr, successStyle.Render("✓ ")+msg)
}

func Info(msg string) {
	fmt.Fprintln(os.Stderr, dimStyle.Render("  "+msg))
}

func Error(msg string) {
	errStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#EF4444")).
		Bold(true)
	fmt.Fprintln(os.Stderr, errStyle.Render("✗ ")+msg)
}

type SpinnerModel struct {
	spinner spinner.Model
	message string
	done    bool
	result  string
	err     error
	task    func() (string, error)
}

type doneMsg struct {
	result string
	err    error
}

func NewSpinner(message string, task func() (string, error)) SpinnerModel {
	s := spinner.New()
	s.Spinner = spinner.Dot
	s.Style = lipgloss.NewStyle().Foreground(accent)

	return SpinnerModel{
		spinner: s,
		message: message,
		task:    task,
	}
}

func (m SpinnerModel) Init() tea.Cmd {
	return tea.Batch(
		m.spinner.Tick,
		func() tea.Msg {
			result, err := m.task()
			return doneMsg{result: result, err: err}
		},
	)
}

func (m SpinnerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case doneMsg:
		m.done = true
		m.result = msg.result
		m.err = msg.err
		return m, tea.Quit

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	}

	return m, nil
}

func (m SpinnerModel) View() string {
	if m.done {
		return ""
	}
	content := m.spinner.View() + "  " + m.message
	return boxStyle.Render(content) + "\n"
}

func RunWithSpinner(message string, task func() (string, error)) (string, error) {
	m := NewSpinner(message, task)

	p := tea.NewProgram(m, tea.WithOutput(os.Stderr)) // 🔥 CRITICAL

	final, err := p.Run()
	if err != nil {
		return "", err
	}

	result := final.(SpinnerModel)
	return result.result, result.err
}
