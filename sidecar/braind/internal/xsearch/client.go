// Package xsearch wraps X API v2 recent search.
package xsearch

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

type Client struct {
	Base   string // e.g. https://api.x.com/2
	Bearer string
	HTTP   *http.Client
}

func New(base, bearer string) *Client {
	return &Client{
		Base:   strings.TrimRight(base, "/"),
		Bearer: bearer,
		HTTP:   &http.Client{Timeout: 20 * time.Second},
	}
}

type Post struct {
	ID        string
	Text      string
	AuthorID  string
	Username  string
	CreatedAt time.Time
	LikeCount int
	RTCount   int
	ReplyCount int
	QuoteCount int
	MediaURL  string
	SourceURL string
	Score     float64
}

type searchResponse struct {
	Data []struct {
		ID               string `json:"id"`
		Text             string `json:"text"`
		AuthorID         string `json:"author_id"`
		CreatedAt        string `json:"created_at"`
		PublicMetrics    *struct {
			LikeCount    int `json:"like_count"`
			RetweetCount int `json:"retweet_count"`
			ReplyCount   int `json:"reply_count"`
			QuoteCount   int `json:"quote_count"`
		} `json:"public_metrics"`
		Attachments *struct {
			MediaKeys []string `json:"media_keys"`
		} `json:"attachments"`
	} `json:"data"`
	Includes *struct {
		Users []struct {
			ID       string `json:"id"`
			Username string `json:"username"`
		} `json:"users"`
		Media []struct {
			MediaKey        string `json:"media_key"`
			Type            string `json:"type"`
			URL             string `json:"url"`
			PreviewImageURL string `json:"preview_image_url"`
		} `json:"media"`
	} `json:"includes"`
	Errors []struct {
		Detail string `json:"detail"`
		Title  string `json:"title"`
	} `json:"errors"`
	Meta struct {
		ResultCount int `json:"result_count"`
	} `json:"meta"`
}

// SearchRecent calls GET /2/tweets/search/recent.
// Returns (nil, ErrNoBearer) when bearer is empty — caller should HOLD/skip.
var ErrNoBearer = fmt.Errorf("X_BEARER_TOKEN missing")

func (c *Client) SearchRecent(query string, maxResults int) ([]Post, error) {
	if strings.TrimSpace(c.Bearer) == "" {
		return nil, ErrNoBearer
	}
	if maxResults < 10 {
		maxResults = 10
	}
	if maxResults > 100 {
		maxResults = 100
	}
	q := url.Values{}
	q.Set("query", query)
	q.Set("max_results", fmt.Sprintf("%d", maxResults))
	q.Set("tweet.fields", "created_at,public_metrics,author_id,attachments")
	q.Set("expansions", "author_id,attachments.media_keys")
	q.Set("user.fields", "username")
	q.Set("media.fields", "url,preview_image_url,type")

	u := c.Base + "/tweets/search/recent?" + q.Encode()
	req, err := http.NewRequest(http.MethodGet, u, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+c.Bearer)
	res, err := c.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(res.Body, 4<<20))
	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("x search %s: %s", res.Status, truncate(string(body), 300))
	}
	var sr searchResponse
	if err := json.Unmarshal(body, &sr); err != nil {
		return nil, err
	}
	users := map[string]string{}
	media := map[string]string{}
	if sr.Includes != nil {
		for _, u := range sr.Includes.Users {
			users[u.ID] = u.Username
		}
		for _, m := range sr.Includes.Media {
			url := m.URL
			if url == "" {
				url = m.PreviewImageURL
			}
			media[m.MediaKey] = url
		}
	}
	out := make([]Post, 0, len(sr.Data))
	for _, t := range sr.Data {
		p := Post{
			ID:       t.ID,
			Text:     t.Text,
			AuthorID: t.AuthorID,
			Username: users[t.AuthorID],
		}
		if t.CreatedAt != "" {
			if ts, err := time.Parse(time.RFC3339, t.CreatedAt); err == nil {
				p.CreatedAt = ts
			}
		}
		if t.PublicMetrics != nil {
			p.LikeCount = t.PublicMetrics.LikeCount
			p.RTCount = t.PublicMetrics.RetweetCount
			p.ReplyCount = t.PublicMetrics.ReplyCount
			p.QuoteCount = t.PublicMetrics.QuoteCount
		}
		if t.Attachments != nil {
			for _, mk := range t.Attachments.MediaKeys {
				if u, ok := media[mk]; ok && u != "" {
					p.MediaURL = u
					break
				}
			}
		}
		if p.Username != "" {
			p.SourceURL = fmt.Sprintf("https://x.com/%s/status/%s", p.Username, p.ID)
		} else {
			p.SourceURL = fmt.Sprintf("https://x.com/i/web/status/%s", p.ID)
		}
		p.Score = Score(p)
		out = append(out, p)
	}
	return out, nil
}

// Score is a public-metrics proxy of the open For You combiner shape.
func Score(p Post) float64 {
	// weighted action proxies
	raw := float64(p.LikeCount) +
		2.0*float64(p.RTCount) +
		1.5*float64(p.ReplyCount) +
		1.2*float64(p.QuoteCount)
	// mild age decay (half-life ~6h)
	ageH := time.Since(p.CreatedAt).Hours()
	if p.CreatedAt.IsZero() {
		ageH = 0
	}
	decay := 1.0
	if ageH > 0 {
		decay = 1.0 / (1.0 + ageH/6.0)
	}
	return raw * decay
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "…"
}
