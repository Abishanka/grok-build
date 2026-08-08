package xaisearch_test

import (
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/xaisearch"
)

func TestSearchNoKey(t *testing.T) {
	c := xaisearch.New("https://api.x.ai/v1", "", "grok-4.5", time.Second)
	f, err := c.Search("ctx", "q")
	if err != nil || f != nil {
		t.Fatalf("want nil,nil got %v %v", f, err)
	}
}

func TestSearchMockWithCitations(t *testing.T) {
	var gotBody map[string]any
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/responses" && !strings.HasSuffix(r.URL.Path, "/responses") {
			http.NotFound(w, r)
			return
		}
		if !strings.HasPrefix(r.Header.Get("Authorization"), "Bearer test-key") {
			w.WriteHeader(401)
			return
		}
		raw, _ := io.ReadAll(r.Body)
		_ = json.Unmarshal(raw, &gotBody)
		// ensure x_search tool requested
		tools, _ := gotBody["tools"].([]any)
		if len(tools) == 0 {
			t.Error("missing tools")
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"id":     "resp_1",
			"status": "completed",
			"output": []map[string]any{
				{
					"type": "message",
					"role": "assistant",
					"content": []map[string]any{
						{
							"type": "output_text",
							"text": "- ratatui layout tip from @tui_dev https://x.com/tui_dev/status/999\n- another angle on braind",
							"annotations": []map[string]any{
								{"type": "url_citation", "url": "https://x.com/tui_dev/status/999", "start_index": 0, "end_index": 10},
							},
						},
					},
				},
			},
		})
	}))
	defer srv.Close()

	c := xaisearch.New(srv.URL, "test-key", "grok-4.5", 5*time.Second)
	findings, err := c.Search("shipping ratatui chrome", "(ratatui OR tui) -is:retweet lang:en")
	if err != nil {
		t.Fatal(err)
	}
	if len(findings) < 1 {
		t.Fatalf("findings: %#v", findings)
	}
	f0 := findings[0]
	if f0.SourceURL != "https://x.com/tui_dev/status/999" {
		t.Fatalf("source: %q", f0.SourceURL)
	}
	if f0.Generated {
		t.Fatal("status URL finding should be generated=false")
	}
	// scrub: secrets in context must not leave
	input, _ := gotBody["input"].([]any)
	if len(input) > 0 {
		msg, _ := input[0].(map[string]any)
		content, _ := msg["content"].(string)
		if strings.Contains(content, "sk-abcdefghijklmnopqrstuvwxyz") {
			t.Fatal("secret leaked into xAI request")
		}
	}

	cards := xaisearch.ToCards(findings, []string{"ratatui"})
	if len(cards) < 1 {
		t.Fatal("no cards")
	}
	if cards[0].Plane != "research" {
		t.Fatalf("plane %q", cards[0].Plane)
	}
	if cards[0].SourceURL == "" {
		t.Fatal("missing source_url on card")
	}
	if cards[0].Generated {
		t.Fatal("real status card should not be generated")
	}
}

func TestSearchMockDigestOnly(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewEncoder(w).Encode(map[string]any{
			"output": []map[string]any{
				{
					"type": "message",
					"content": []map[string]any{
						{"type": "output_text", "text": "Builders are discussing debounce windows for tip intake and rank caps."},
					},
				},
			},
		})
	}))
	defer srv.Close()

	c := xaisearch.New(srv.URL, "k", "grok-4.5", time.Second)
	findings, err := c.Search("braind worker", "braind")
	if err != nil {
		t.Fatal(err)
	}
	if len(findings) != 1 || !findings[0].Generated {
		t.Fatalf("%#v", findings)
	}
	cards := xaisearch.ToCards(findings, nil)
	if len(cards) != 1 || !cards[0].Generated || cards[0].Kind != "digest" {
		t.Fatalf("%#v", cards)
	}
}

func TestSearchHTTPError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(410)
		_, _ = w.Write([]byte(`{"error":"live_search gone"}`))
	}))
	defer srv.Close()
	c := xaisearch.New(srv.URL, "k", "m", time.Second)
	_, err := c.Search("c", "q")
	if err == nil || !strings.Contains(err.Error(), "410") {
		t.Fatalf("err=%v", err)
	}
}

func TestScrubSecretsInRequest(t *testing.T) {
	var saw string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		b, _ := io.ReadAll(r.Body)
		saw = string(b)
		_ = json.NewEncoder(w).Encode(map[string]any{
			"output": []map[string]any{
				{"content": []map[string]any{{"text": "ok nothing much"}}},
			},
		})
	}))
	defer srv.Close()
	c := xaisearch.New(srv.URL, "k", "m", time.Second)
	secret := "sk-" + strings.Repeat("x", 20)
	_, err := c.Search("leaked "+secret+" in log", "query")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(saw, secret) {
		t.Fatal("API key-shaped secret left the machine")
	}
	if !strings.Contains(saw, "[REDACTED]") {
		t.Fatalf("expected redaction marker in body: %s", saw)
	}
}
