package test

import (
	"os"
	"testing"

	"github.com/jotx19/jotline/internal/extractor"
)

func TestExtractTxt(t *testing.T) {
	f, _ := os.CreateTemp("", "test*.txt")
	f.WriteString("Bonjour, ceci est un test en français.")
	f.Close()
	defer os.Remove(f.Name())

	result, err := extractor.Extract(f.Name())
	if err != nil {
		t.Fatal(err)
	}

	if result.RawText == "" {
		t.Fatal("expected text, got empty string")
	}

	t.Logf("✅ Title: %s", result.Title)
	t.Logf("✅ Text: %s", result.RawText)
}
