package worker_test

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/config"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/worker"
)

// end-to-end: mock jamd + mock X API → research cards POSTed back
func TestWorkerRunNowPostsResearchCards(t *testing.T) {
	var mu sync.Mutex
	var posted []map[string]any
	var xHits int

	mux := http.NewServeMux()
	// jamd routes
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "jam_id": "demo"})
	})
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id":         "demo",
			"context_union":  "host[active]: shipping braind · abi[waiting]: ratatui chrome",
			"topic_keys":     []string{"braind", "ratatui"},
			"members":        []any{},
			"repo_url":       "",
			"default_branch": "main",
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo",
			"tips": []map[string]any{
				{"origin": "host", "ts": 100.0, "context": "shipping braind", "topic_keys": []string{"braind"}},
				{"origin": "abi", "ts": 101.0, "context": "ratatui chrome", "topic_keys": []string{"ratatui"}},
			},
			"count": 2,
		})
	})
	mux.HandleFunc("/jam/demo/cards", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method", 405)
			return
		}
		if r.Header.Get("X-Jam-Token") != "dev" {
			w.WriteHeader(401)
			_ = json.NewEncoder(w).Encode(map[string]any{"ok": false})
			return
		}
		var body struct {
			Cards []map[string]any `json:"cards"`
		}
		_ = json.NewDecoder(r.Body).Decode(&body)
		mu.Lock()
		posted = append(posted, body.Cards...)
		mu.Unlock()
		// echo with seq
		out := make([]map[string]any, 0, len(body.Cards))
		for i, c := range body.Cards {
			c["id"] = "c" + string(rune('1'+i))
			c["seq"] = i + 1
			out = append(out, c)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "cards": out, "count": len(out)})
	})

	// X API
	mux.HandleFunc("/tweets/search/recent", func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		xHits++
		mu.Unlock()
		if r.Header.Get("Authorization") != "Bearer x-tok" {
			w.WriteHeader(401)
			return
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"data": []map[string]any{
				{
					"id": "555", "text": "braind tips → research cards", "author_id": "9",
					"created_at": time.Now().UTC().Format(time.RFC3339),
					"public_metrics": map[string]int{"like_count": 7, "retweet_count": 1, "reply_count": 0, "quote_count": 0},
				},
			},
			"includes": map[string]any{
				"users": []map[string]any{{"id": "9", "username": "buildbot"}},
			},
		})
	})

	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:        srv.URL,
		JamID:          "demo",
		JamToken:       "dev",
		XBearerToken:   "x-tok",
		XSearchBaseURL: srv.URL, // same server hosts mock X path
		MaxCards:       3,
		Debounce:       10 * time.Second,
		PollInterval:   time.Second,
	}
	wkr := worker.New(cfg)
	if err := wkr.RunNow(); err != nil {
		t.Fatal(err)
	}

	mu.Lock()
	defer mu.Unlock()
	if xHits != 1 {
		t.Fatalf("x hits %d", xHits)
	}
	if len(posted) < 1 {
		t.Fatal("no cards posted")
	}
	c0 := posted[0]
	if c0["plane"] != "research" {
		t.Fatalf("plane: %#v", c0)
	}
	if c0["generated"] != false {
		t.Fatalf("generated: %#v", c0["generated"])
	}
	src, _ := c0["source_url"].(string)
	if !strings.Contains(src, "555") {
		t.Fatalf("source_url: %#v", c0["source_url"])
	}
}

func TestWorkerHoldWithoutXBearer(t *testing.T) {
	var posted int
	mux := http.NewServeMux()
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo", "context_union": "x", "topic_keys": []string{"go"},
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"tips": []any{}})
	})
	mux.HandleFunc("/jam/demo/cards", func(w http.ResponseWriter, r *http.Request) {
		posted++
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "cards": []any{}})
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:        srv.URL,
		JamID:          "demo",
		JamToken:       "dev",
		XBearerToken:   "", // missing
		XSearchBaseURL: srv.URL,
		MaxCards:       3,
		Debounce:       time.Millisecond,
	}
	wkr := worker.New(cfg)
	if err := wkr.RunNow(); err != nil {
		t.Fatal(err)
	}
	if posted != 0 {
		t.Fatalf("should not post on HOLD, posted=%d", posted)
	}
}

func TestDebounceCoalesce(t *testing.T) {
	var runs int
	mux := http.NewServeMux()
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo", "context_union": "host: a", "topic_keys": []string{"go"},
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"tips": []map[string]any{
				{"origin": "host", "ts": float64(time.Now().Unix()), "context": "tip", "topic_keys": []string{"go"}},
			},
		})
	})
	mux.HandleFunc("/tweets/search/recent", func(w http.ResponseWriter, r *http.Request) {
		runs++
		_ = json.NewEncoder(w).Encode(map[string]any{"data": []any{}})
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:        srv.URL,
		JamID:          "demo",
		XBearerToken:   "t",
		XSearchBaseURL: srv.URL,
		Debounce:       200 * time.Millisecond,
		MaxCards:       2,
	}
	wkr := worker.New(cfg)

	ran, err := wkr.Tick()
	if err != nil {
		t.Fatal(err)
	}
	if !ran {
		t.Fatal("first tick should run")
	}
	// immediate second tick within debounce → no run
	ran, err = wkr.Tick()
	if err != nil {
		t.Fatal(err)
	}
	if ran {
		t.Fatal("second tick should debounce")
	}
	if runs != 1 {
		t.Fatalf("x runs %d want 1", runs)
	}
}

func TestXAISearchBackground(t *testing.T) {
	var mu sync.Mutex
	var posted []map[string]any
	var xaiHits int

	mux := http.NewServeMux()
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo", "topic_keys": []string{"ratatui"}, "context_union": "building freeze",
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"tips": []any{}})
	})
	mux.HandleFunc("/jam/demo/cards", func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			Cards []map[string]any `json:"cards"`
		}
		_ = json.NewDecoder(r.Body).Decode(&body)
		mu.Lock()
		posted = append(posted, body.Cards...)
		mu.Unlock()
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "cards": body.Cards})
	})
	mux.HandleFunc("/tweets/search/recent", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"data": []map[string]any{
				{"id": "42", "text": "ratatui rocks", "author_id": "u",
					"created_at": time.Now().UTC().Format(time.RFC3339),
					"public_metrics": map[string]int{"like_count": 4}},
			},
			"includes": map[string]any{"users": []map[string]any{{"id": "u", "username": "tuidev"}}},
		})
	})
	mux.HandleFunc("/responses", func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		xaiHits++
		mu.Unlock()
		_ = json.NewEncoder(w).Encode(map[string]any{
			"output": []map[string]any{
				{
					"type": "message",
					"content": []map[string]any{
						{
							"type": "output_text",
							"text": "Deep dive on ratatui layouts https://x.com/deep/status/777",
							"annotations": []map[string]any{
								{"type": "url_citation", "url": "https://x.com/deep/status/777"},
							},
						},
					},
				},
			},
		})
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:           srv.URL,
		JamID:             "demo",
		JamToken:          "dev",
		XBearerToken:      "t",
		XSearchBaseURL:    srv.URL,
		XAIAPIKey:         "xai-test",
		XAISearchURL:      srv.URL + "/responses",
		EnableXAISearch:   true,
		XAISearchTimeout:  5 * time.Second,
		MaxCards:          3,
		Debounce:          time.Millisecond,
	}
	wkr := worker.New(cfg)
	if err := wkr.RunNow(); err != nil {
		t.Fatal(err)
	}
	wkr.WaitBackground()

	mu.Lock()
	defer mu.Unlock()
	if xaiHits != 1 {
		t.Fatalf("xai hits %d", xaiHits)
	}
	foundRecent := false
	foundXAI := false
	for _, c := range posted {
		src, _ := c["source_url"].(string)
		if strings.Contains(src, "42") {
			foundRecent = true
		}
		if strings.Contains(src, "777") {
			foundXAI = true
			if c["plane"] != "research" {
				t.Fatalf("xai card plane: %#v", c)
			}
		}
	}
	if !foundRecent {
		t.Fatalf("missing recent-search card: %#v", posted)
	}
	if !foundXAI {
		t.Fatalf("missing x_search card: %#v", posted)
	}
}

func TestXAISearchDisabledByDefault(t *testing.T) {
	var xaiHits int
	mux := http.NewServeMux()
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo", "topic_keys": []string{"go"}, "context_union": "x",
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"tips": []any{}})
	})
	mux.HandleFunc("/jam/demo/cards", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "cards": []any{}})
	})
	mux.HandleFunc("/tweets/search/recent", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"data": []any{}})
	})
	mux.HandleFunc("/responses", func(w http.ResponseWriter, r *http.Request) {
		xaiHits++
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:        srv.URL,
		JamID:          "demo",
		XBearerToken:   "t",
		XSearchBaseURL: srv.URL,
		XAIAPIKey:      "xai-test",
		XAISearchURL:   srv.URL + "/responses",
		EnableXAISearch: false, // default off
		MaxCards:       2,
	}
	wkr := worker.New(cfg)
	_ = wkr.RunNow()
	wkr.WaitBackground()
	if xaiHits != 0 {
		t.Fatalf("x_search should be off, hits=%d", xaiHits)
	}
}

func TestDigestOptional(t *testing.T) {
	var posted []map[string]any
	mux := http.NewServeMux()
	mux.HandleFunc("/jam/demo/meta", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"jam_id": "demo", "topic_keys": []string{"go"}, "context_union": "building",
		})
	})
	mux.HandleFunc("/jam/demo/tips", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{"tips": []any{}})
	})
	mux.HandleFunc("/jam/demo/cards", func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			Cards []map[string]any `json:"cards"`
		}
		_ = json.NewDecoder(r.Body).Decode(&body)
		posted = append(posted, body.Cards...)
		_ = json.NewEncoder(w).Encode(map[string]any{"ok": true, "cards": body.Cards})
	})
	mux.HandleFunc("/tweets/search/recent", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"data": []map[string]any{
				{"id": "1", "text": "go tip", "author_id": "u", "created_at": time.Now().UTC().Format(time.RFC3339),
					"public_metrics": map[string]int{"like_count": 3}},
			},
			"includes": map[string]any{"users": []map[string]any{{"id": "u", "username": "gopher"}}},
		})
	})
	mux.HandleFunc("/chat/completions", func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"choices": []map[string]any{
				{"message": map[string]string{"content": "Room is shipping Go tooling; X is talking about tips."}},
			},
		})
	})
	srv := httptest.NewServer(mux)
	defer srv.Close()

	cfg := config.Config{
		JamHost:        srv.URL,
		JamID:          "demo",
		JamToken:       "",
		XBearerToken:   "t",
		XSearchBaseURL: srv.URL,
		XAIAPIKey:      "xai-test",
		XAIChatURL:     srv.URL + "/chat/completions",
		XAIModel:       "grok-4.5",
		MaxCards:       3,
		Debounce:       time.Millisecond,
	}
	wkr := worker.New(cfg)
	if err := wkr.RunNow(); err != nil {
		t.Fatal(err)
	}
	foundDigest := false
	foundReal := false
	for _, c := range posted {
		if c["kind"] == "digest" && c["generated"] == true {
			foundDigest = true
		}
		if c["kind"] == "real" && c["plane"] == "research" {
			foundReal = true
		}
	}
	if !foundReal {
		t.Fatalf("missing real research card: %#v", posted)
	}
	if !foundDigest {
		t.Fatalf("missing digest: %#v", posted)
	}
}
