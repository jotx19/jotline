package extractor

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/ledongthuc/pdf"
	"github.com/lu4p/cat"
)

type Result struct {
	Title    string
	RawText  string
	FileType string
	FileName string
	Size     int64
}

func Extract(filePath string) (*Result, error) {
	info, err := os.Stat(filePath)
	if err != nil {
		return nil, fmt.Errorf("file not found: %s", filePath)
	}

	ext := strings.ToLower(filepath.Ext(filePath))
	fileName := filepath.Base(filePath)
	title := strings.TrimSuffix(fileName, filepath.Ext(fileName))

	var rawText string
	var fileType string

	switch ext {
	case ".pdf":
		fileType = "pdf"
		rawText, err = extractPDF(filePath)
		if err != nil {
			return nil, fmt.Errorf("failed to extract PDF: %w", err)
		}

	case ".docx":
		fileType = "docx"
		rawText, err = cat.File(filePath)
		if err != nil {
			return nil, fmt.Errorf("failed to extract DOCX: %w", err)
		}

	case ".txt", ".md", ".markdown":
		fileType = ext[1:]
		data, err := os.ReadFile(filePath)
		if err != nil {
			return nil, fmt.Errorf("failed to read file: %w", err)
		}
		rawText = string(data)

	default:
		return nil, fmt.Errorf(
			"unsupported file type %q — supported: .pdf .docx .txt .md",
			ext,
		)
	}

	rawText = clean(rawText)

	if rawText == "" {
		return nil, fmt.Errorf("no text could be extracted from %s", fileName)
	}

	return &Result{
		Title:    title,
		RawText:  rawText,
		FileType: fileType,
		FileName: fileName,
		Size:     info.Size(),
	}, nil
}

func extractPDF(filePath string) (string, error) {
	f, r, err := pdf.Open(filePath)
	if err != nil {
		return "", err
	}
	defer f.Close()

	var buf bytes.Buffer
	b, err := r.GetPlainText()
	if err != nil {
		return "", err
	}
	buf.ReadFrom(b)
	return buf.String(), nil
}

func clean(s string) string {
	s = strings.ReplaceAll(s, "\r\n", "\n")
	for strings.Contains(s, "\n\n\n") {
		s = strings.ReplaceAll(s, "\n\n\n", "\n\n")
	}
	return strings.TrimSpace(s)
}
