// Package jamclient talks to jamd over HTTP (tips in, research cards out).
package jamclient

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

type Client struct {
	Base   string
	JamID  string
	Token  string
	HTTP   *http.Client
}

func New(base, jamID, token string) *Client {
	return &Client{
		Base:  strings.TrimRight(base, "/"),
		JamID: jamID,
		Token: token,
		HTTP:  &http.Client{Timeout: 15 * time.Second},
	}
}

type Meta struct {
	JamID         string   `json:"jam_id"`
	ContextUnion  string   `json:"context_union"`
	TopicKeys     []string `json:"topic_keys"`
	Members       []Tip    `json:"members"`
	RepoURL       string   `json:"repo_url"`
	DefaultBranch string   `json:"default_branch"`
}

// Tip is a jam context tip (product law: jam stores tips, not git wrappers).
type Tip struct {
	Origin       string   `json:"origin"`
	TS           float64  `json:"ts"`
	Mode         string   `json:"mode"`
	Context      string   `json:"context"`
	TopicKeys    []string `json:"topic_keys"`
	Branch       string   `json:"branch"`
	SHA          string   `json:"sha"`
	Chat         string   `json:"chat"`
	DirtySummary string   `json:"dirty_summary"`
}

type Card struct {
	ID            string   `json:"id,omitempty"`
	Seq           int      `json:"seq,omitempty"`
	Kind          string   `json:"kind"`
	Title         string   `json:"title"`
	Body          string   `json:"body"`
	MediaURL      string   `json:"media_url,omitempty"`
	SourceURL     string   `json:"source_url,omitempty"`
	Generated     bool     `json:"generated"`
	Justification string   `json:"justification,omitempty"`
	DiffContext   string   `json:"diff_context,omitempty"`
	Plane         string   `json:"plane,omitempty"`
	JamOrigin     string   `json:"jam_origin,omitempty"`
	TopicKeys     []string `json:"topic_keys,omitempty"`
}

func (c *Client) Health() error {
	res, err := c.HTTP.Get(c.Base + "/health")
	if err != nil {
		return err
	}
	defer res.Body.Close()
	if res.StatusCode >= 300 {
		return fmt.Errorf("health %s", res.Status)
	}
	return nil
}

func (c *Client) Meta() (Meta, error) {
	var m Meta
	url := fmt.Sprintf("%s/jam/%s/meta", c.Base, urlPath(c.JamID))
	res, err := c.HTTP.Get(url)
	if err != nil {
		return m, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if res.StatusCode >= 300 {
		return m, fmt.Errorf("meta %s: %s", res.Status, truncate(string(body), 200))
	}
	if err := json.Unmarshal(body, &m); err != nil {
		return m, err
	}
	return m, nil
}

func (c *Client) Tips(sinceTS float64) ([]Tip, error) {
	u := fmt.Sprintf("%s/jam/%s/tips?since_ts=%s", c.Base, urlPath(c.JamID),
		url.QueryEscape(strconv.FormatFloat(sinceTS, 'f', -1, 64)))
	res, err := c.HTTP.Get(u)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("tips %s: %s", res.Status, truncate(string(body), 200))
	}
	var out struct {
		Tips []Tip `json:"tips"`
	}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out.Tips, nil
}

func (c *Client) PostCards(cards []Card) ([]Card, error) {
	payload := map[string]any{"cards": cards}
	if c.Token != "" {
		payload["token"] = c.Token
	}
	b, _ := json.Marshal(payload)
	u := fmt.Sprintf("%s/jam/%s/cards", c.Base, urlPath(c.JamID))
	req, err := http.NewRequest(http.MethodPost, u, bytes.NewReader(b))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	if c.Token != "" {
		req.Header.Set("X-Jam-Token", c.Token)
	}
	res, err := c.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("post cards %s: %s", res.Status, truncate(string(body), 200))
	}
	var out struct {
		OK    bool   `json:"ok"`
		Cards []Card `json:"cards"`
	}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out.Cards, nil
}

func urlPath(s string) string {
	return url.PathEscape(s)
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "…"
}
