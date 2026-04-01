package test

import (
	"testing"

	"github.com/jotx19/jotline/internal/config"
)

func TestSaveAndLoad(t *testing.T) {
	// Save a config
	cfg := &config.Config{
		MongoURI: "mongodb+srv://test:test@cluster.mongodb.net",
		Database: "pipeline-db",
	}

	err := config.Save(cfg)
	if err != nil {
		t.Fatal("Save failed:", err)
	}

	// Load it back
	loaded, err := config.Load()
	if err != nil {
		t.Fatal("Load failed:", err)
	}

	if loaded.MongoURI != cfg.MongoURI {
		t.Fatalf("expected %s got %s", cfg.MongoURI, loaded.MongoURI)
	}

	t.Logf("✅ MongoURI: %s", loaded.MongoURI)
	t.Logf("✅ Database: %s", loaded.Database)
}
