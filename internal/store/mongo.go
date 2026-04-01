package store

import (
	"context"
	"fmt"
	"time"

	"go.mongodb.org/mongo-driver/v2/bson"
	"go.mongodb.org/mongo-driver/v2/mongo"
	"go.mongodb.org/mongo-driver/v2/mongo/options"
)

type Document struct {
	ID        bson.ObjectID `bson:"_id,omitempty"`
	Title     string        `bson:"title"`
	FileName  string        `bson:"file_name"`
	FileType  string        `bson:"file_type"`
	RawText   string        `bson:"raw_text"`
	Size      int64         `bson:"size"`
	CharCount int           `bson:"char_count"`
	CreatedAt time.Time     `bson:"created_at"`
}

type Store struct {
	col *mongo.Collection
}

func New(uri, dbName string) (*Store, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	client, err := mongo.Connect(options.Client().ApplyURI(uri))
	if err != nil {
		return nil, fmt.Errorf("failed to connect: %w", err)
	}

	if err := client.Ping(ctx, nil); err != nil {
		return nil, fmt.Errorf("connection failed: %w", err)
	}

	col := client.Database(dbName).Collection("documents")

	// text index for search
	_, _ = col.Indexes().CreateOne(ctx, mongo.IndexModel{
		Keys: bson.D{
			{Key: "title", Value: "text"},
			{Key: "raw_text", Value: "text"},
		},
	})

	return &Store{col: col}, nil
}

func TestConnection(uri string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	defer cancel()
	client, err := mongo.Connect(options.Client().ApplyURI(uri))
	if err != nil {
		return err
	}
	return client.Ping(ctx, nil)
}

func (s *Store) Save(doc *Document) error {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	doc.ID = bson.NewObjectID()
	doc.CreatedAt = time.Now()
	doc.CharCount = len(doc.RawText)
	_, err := s.col.InsertOne(ctx, doc)
	return err
}

func (s *Store) List() ([]Document, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	opts := options.Find().
		SetSort(bson.D{{Key: "created_at", Value: -1}}).
		SetProjection(bson.D{{Key: "raw_text", Value: 0}})
	cur, err := s.col.Find(ctx, bson.D{}, opts)
	if err != nil {
		return nil, err
	}
	defer cur.Close(ctx)
	var docs []Document
	return docs, cur.All(ctx, &docs)
}

func (s *Store) Delete(id string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	objID, err := bson.ObjectIDFromHex(id)
	if err != nil {
		return fmt.Errorf("invalid ID: %s", id)
	}
	res, err := s.col.DeleteOne(ctx, bson.D{{Key: "_id", Value: objID}})
	if err != nil {
		return err
	}
	if res.DeletedCount == 0 {
		return fmt.Errorf("document not found: %s", id)
	}
	return nil
}

func (s *Store) Search(query string) ([]Document, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	filter := bson.D{{Key: "$text", Value: bson.D{{Key: "$search", Value: query}}}}
	opts := options.Find().SetSort(bson.D{{Key: "score", Value: bson.D{{Key: "$meta", Value: "textScore"}}}})
	cur, err := s.col.Find(ctx, filter, opts)
	if err != nil {
		return nil, err
	}
	defer cur.Close(ctx)
	var docs []Document
	return docs, cur.All(ctx, &docs)
}

func (s *Store) GetByID(id string) (*Document, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	objID, err := bson.ObjectIDFromHex(id)
	if err != nil {
		return nil, fmt.Errorf("invalid ID: %s", id)
	}
	var doc Document
	err = s.col.FindOne(ctx, bson.D{{Key: "_id", Value: objID}}).Decode(&doc)
	return &doc, err
}

func (s *Store) All() ([]Document, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	cur, err := s.col.Find(ctx, bson.D{})
	if err != nil {
		return nil, err
	}
	defer cur.Close(ctx)
	var docs []Document
	return docs, cur.All(ctx, &docs)
}
