// Package digest optionally builds one AI digest card via xAI chat.
package digest

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/scrub"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/xsearch"
)

type Client struct {
	URL    string // full chat completions URL
	APIKey string
	Model  string
	HTTP   *http.Client
}

func New(url, apiKey, model string) *Client {
	return &Client{
		URL:    url,
		APIKey: apiKey,
		Model:  model,
		HTTP:   &http.Client{Timeout: 45 * time.Second},
	}
}

// Digest returns nil,nil when API key missing (skip gracefully).
func (c *Client) Digest(contextUnion string, posts []xsearch.Post) (*jamclient.Card, error) {
	if c == nil || strings.TrimSpace(c.APIKey) == "" {
		return nil, nil
	}
	if len(posts) == 0 {
		return nil, nil
	}
	var b strings.Builder
	b.WriteString("Room context: ")
	b.WriteString(scrub.Text(clamp(contextUnion, 400)))
	b.WriteString("\n\nTop posts:\n")
	for i, p := range posts {
		if i >= 5 {
			break
		}
		who := p.Username
		if who == "" {
			who = "?"
		}
		fmt.Fprintf(&b, "- @%s: %s\n", who, scrub.Text(clamp(p.Text, 200)))
	}
	b.WriteString("\nWrite a 2-sentence research digest for coders in this jam. No hashtags. No URLs.")

	payload := map[string]any{
		"model": c.Model,
		"messages": []map[string]string{
			{"role": "system", "content": "You summarize X research for a coding jam. Be concrete and short."},
			{"role": "user", "content": b.String()},
		},
		"temperature": 0.4,
		"max_tokens":  220,
	}
	raw, _ := json.Marshal(payload)
	req, err := http.NewRequest(http.MethodPost, c.URL, bytes.NewReader(raw))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.APIKey)
	res, err := c.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("xai chat %s: %s", res.Status, clamp(string(body), 200))
	}
	var parsed struct {
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := json.Unmarshal(body, &parsed); err != nil {
		return nil, err
	}
	if len(parsed.Choices) == 0 || strings.TrimSpace(parsed.Choices[0].Message.Content) == "" {
		return nil, nil
	}
	text := strings.TrimSpace(parsed.Choices[0].Message.Content)
	return &jamclient.Card{
		Kind:          "digest",
		Title:         "research digest",
		Body:          clamp(text, 800),
		Generated:     true,
		Justification: "xAI chat digest of ranked X posts",
		Plane:         "research",
	}, nil
}

func clamp(s string, n int) string {
	r := []rune(strings.TrimSpace(s))
	if len(r) <= n {
		return string(r)
	}
	return string(r[:n-3]) + "..."
}
