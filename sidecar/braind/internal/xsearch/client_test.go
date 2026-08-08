package xsearch_test

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/xsearch"
)

func TestSearchRecentMock(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/tweets/search/recent" {
			http.NotFound(w, r)
			return
		}
		auth := r.Header.Get("Authorization")
		if !strings.HasPrefix(auth, "Bearer test-token") {
			w.WriteHeader(401)
			return
		}
		q := r.URL.Query().Get("query")
		if q == "" {
			t.Error("missing query")
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"data": []map[string]any{
				{
					"id":        "123",
					"text":      "hello ratatui world",
					"author_id": "u1",
					"created_at": "2026-08-08T12:00:00.000Z",
					"public_metrics": map[string]int{
						"like_count":    10,
						"retweet_count": 2,
						"reply_count":   1,
						"quote_count":   0,
					},
				},
			},
			"includes": map[string]any{
				"users": []map[string]any{
					{"id": "u1", "username": "coder"},
				},
			},
			"meta": map[string]int{"result_count": 1},
		})
	}))
	defer srv.Close()

	c := xsearch.New(srv.URL, "test-token")
	posts, err := c.SearchRecent("ratatui -is:retweet lang:en", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(posts) != 1 {
		t.Fatalf("posts %d", len(posts))
	}
	p := posts[0]
	if p.Username != "coder" || p.ID != "123" {
		t.Fatalf("%+v", p)
	}
	if p.SourceURL != "https://x.com/coder/status/123" {
		t.Fatalf("source %q", p.SourceURL)
	}
	if p.Score <= 0 {
		t.Fatalf("score %v", p.Score)
	}
}

func TestSearchNoBearer(t *testing.T) {
	c := xsearch.New("https://api.x.com/2", "")
	_, err := c.SearchRecent("x", 10)
	if err != xsearch.ErrNoBearer {
		t.Fatalf("want ErrNoBearer got %v", err)
	}
}

func TestSearchHTTPError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(429)
		_, _ = w.Write([]byte(`{"title":"Too Many Requests"}`))
	}))
	defer srv.Close()
	c := xsearch.New(srv.URL, "tok")
	_, err := c.SearchRecent("go", 10)
	if err == nil || !strings.Contains(err.Error(), "429") {
		t.Fatalf("err=%v", err)
	}
}
