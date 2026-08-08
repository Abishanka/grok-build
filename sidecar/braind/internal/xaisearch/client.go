// Package xaisearch calls xAI Responses API with tools:[{type:x_search}].
// Background semantic path — optional, fail-open, never blocks recent-search.
// Note: tool name live_search is dead (410); use x_search only.
package xaisearch

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/scrub"
)

// Client talks to POST {Base}/responses with x_search tool.
type Client struct {
	URL    string // full responses URL
	APIKey string
	Model  string
	HTTP   *http.Client
}

func New(baseURL, apiKey, model string, timeout time.Duration) *Client {
	if timeout <= 0 {
		timeout = 25 * time.Second
	}
	base := strings.TrimRight(baseURL, "/")
	url := base
	if !strings.HasSuffix(base, "/responses") {
		url = base + "/responses"
	}
	return &Client{
		URL:    url,
		APIKey: apiKey,
		Model:  model,
		HTTP:   &http.Client{Timeout: timeout},
	}
}

// Finding is one mapped research card candidate from x_search.
type Finding struct {
	Title     string
	Body      string
	SourceURL string
	Generated bool
}

var (
	reXStatus = regexp.MustCompile(`(?i)https?://(?:x\.com|twitter\.com)/([A-Za-z0-9_]+)/status/(\d+)`)
	reURL     = regexp.MustCompile(`https?://[^\s\)\]\"']+`)
)

// Search runs one x_search-backed response. Returns nil,nil when key missing or empty.
// Prompt/context are scrubbed before leaving the machine.
func (c *Client) Search(contextUnion, query string) ([]Finding, error) {
	if c == nil || strings.TrimSpace(c.APIKey) == "" {
		return nil, nil
	}
	ctx := scrub.Text(scrub.Clamp(contextUnion, 400))
	q := scrub.Text(scrub.Clamp(query, 400))
	if ctx == "" && q == "" {
		return nil, nil
	}

	user := "You are researching for a coding jam.\n"
	if ctx != "" {
		user += "Room context: " + ctx + "\n"
	}
	if q != "" {
		user += "Search focus: " + q + "\n"
	}
	user += "Use x_search to find recent relevant posts on X about this work. " +
		"Reply with 2-4 short bullets. Prefer real posts; include x.com status URLs when known. " +
		"No hashtags spam. Be concrete for builders."

	payload := map[string]any{
		"model": c.Model,
		"input": []map[string]string{
			{"role": "user", "content": user},
		},
		"tools": []map[string]string{
			{"type": "x_search"},
		},
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
	body, _ := io.ReadAll(io.LimitReader(res.Body, 2<<20))
	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("xai responses %s: %s", res.Status, clamp(string(body), 200))
	}

	text, urls := parseResponse(body)
	if strings.TrimSpace(text) == "" && len(urls) == 0 {
		return nil, nil
	}
	return findingsFrom(text, urls), nil
}

// ToCards maps findings → plane=research cards.
func ToCards(findings []Finding, topicKeys []string) []jamclient.Card {
	cards := make([]jamclient.Card, 0, len(findings))
	for _, f := range findings {
		body := strings.TrimSpace(f.Body)
		if body == "" {
			continue
		}
		title := f.Title
		if title == "" {
			if f.SourceURL != "" {
				title = "x_search find"
			} else {
				title = "x_search digest"
			}
		}
		kind := "digest"
		if f.SourceURL != "" && !f.Generated {
			kind = "real"
		}
		cards = append(cards, jamclient.Card{
			Kind:          kind,
			Title:         title,
			Body:          clamp(body, 480),
			SourceURL:     f.SourceURL,
			Generated:     f.Generated || f.SourceURL == "",
			Justification: "xAI responses x_search background",
			Plane:         "research",
			TopicKeys:     topicKeys,
		})
	}
	return cards
}

// parseResponse extracts assistant text + citation/status URLs from Responses API JSON.
func parseResponse(raw []byte) (string, []string) {
	// Flexible parse: walk output[].content[].text and annotations[].url
	var root map[string]any
	if err := json.Unmarshal(raw, &root); err != nil {
		return "", nil
	}
	var texts []string
	var urls []string
	seenURL := map[string]bool{}

	addURL := func(u string) {
		u = strings.TrimRight(u, ".,);]")
		if u == "" || seenURL[u] {
			return
		}
		seenURL[u] = true
		urls = append(urls, u)
	}

	output, _ := root["output"].([]any)
	for _, item := range output {
		m, ok := item.(map[string]any)
		if !ok {
			continue
		}
		// message content blocks
		if content, ok := m["content"].([]any); ok {
			for _, block := range content {
				b, ok := block.(map[string]any)
				if !ok {
					continue
				}
				if t, ok := b["text"].(string); ok && t != "" {
					texts = append(texts, t)
				}
				if anns, ok := b["annotations"].([]any); ok {
					for _, a := range anns {
						am, _ := a.(map[string]any)
						if u, ok := am["url"].(string); ok {
							addURL(u)
						}
					}
				}
			}
		}
		// some shapes put text at top level
		if t, ok := m["text"].(string); ok && t != "" {
			texts = append(texts, t)
		}
	}
	// also top-level output_text convenience if present
	if t, ok := root["output_text"].(string); ok && t != "" {
		texts = append(texts, t)
	}

	joined := strings.TrimSpace(strings.Join(texts, "\n"))
	// harvest x.com URLs from text too
	for _, m := range reXStatus.FindAllString(joined, 8) {
		addURL(m)
	}
	for _, m := range reURL.FindAllString(joined, 12) {
		if strings.Contains(m, "x.com/") || strings.Contains(m, "twitter.com/") {
			addURL(m)
		}
	}
	return joined, urls
}

func findingsFrom(text string, urls []string) []Finding {
	var out []Finding
	// Prefer one card per x.com status URL with surrounding context.
	xURLs := make([]string, 0, len(urls))
	for _, u := range urls {
		if reXStatus.MatchString(u) {
			xURLs = append(xURLs, u)
		}
	}
	bullets := splitBullets(text)
	if len(xURLs) > 0 {
		for i, u := range xURLs {
			if i >= 4 {
				break
			}
			body := text
			if i < len(bullets) {
				body = bullets[i]
			} else if len(bullets) > 0 {
				body = bullets[0]
			}
			// strip raw URL from body to keep card clean
			body = strings.TrimSpace(reURL.ReplaceAllString(body, ""))
			if body == "" {
				body = "Relevant post via x_search"
			}
			who := ""
			if m := reXStatus.FindStringSubmatch(u); len(m) > 1 {
				who = "@" + m[1]
			}
			title := who
			if title == "" {
				title = "x_search find"
			}
			out = append(out, Finding{
				Title:     title,
				Body:      clamp(body, 480),
				SourceURL: u,
				Generated: false, // real post URL
			})
		}
		return out
	}
	// No status URLs — one generated digest card from the model text.
	if strings.TrimSpace(text) == "" {
		return nil
	}
	body := text
	if len(bullets) > 0 {
		body = strings.Join(bullets, "\n")
	}
	out = append(out, Finding{
		Title:     "x_search digest",
		Body:      clamp(body, 800),
		Generated: true,
	})
	return out
}

func splitBullets(text string) []string {
	var out []string
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		line = strings.TrimLeft(line, "•-–—*0123456789.) ")
		line = strings.TrimSpace(line)
		if len(line) < 8 {
			continue
		}
		out = append(out, line)
		if len(out) >= 6 {
			break
		}
	}
	return out
}

func clamp(s string, n int) string {
	return scrub.Clamp(s, n)
}
