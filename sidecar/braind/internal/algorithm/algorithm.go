// Package algorithm implements the Code Jam content pipeline:
// tips → distill/union → X search/recent → rank → curator → research cards.
package algorithm

import (
	"fmt"
	"sort"
	"strings"
	"unicode"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/scrub"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/xsearch"
)

// Input is one debounced brain run for a jam.
type Input struct {
	Meta     jamclient.Meta
	Tips     []jamclient.Tip
	MaxCards int
}

// Result is curator output for one run.
type Result struct {
	Query     string
	Status    string // ok | hold | skip
	Reason    string
	Cards     []jamclient.Card
	Posts     []xsearch.Post
	Distilled Distilled
}

// Searcher is implemented by xsearch.Client (and mocks).
type Searcher interface {
	SearchRecent(query string, maxResults int) ([]xsearch.Post, error)
}

// Digester optionally produces one AI digest card.
type Digester interface {
	Digest(contextUnion string, posts []xsearch.Post) (*jamclient.Card, error)
}

// BuildQuery turns topic_keys + context_union into an X recent-search query.
// Caps at ~480 chars; pairs standalone terms with -is:retweet lang:en.
// Query text is secret-scrubbed before return.
func BuildQuery(topicKeys []string, contextUnion string) string {
	keys := normalizeKeys(topicKeys, 8)
	if len(keys) == 0 {
		// light extract from union
		keys = extractKeys(contextUnion, 6)
	}
	if len(keys) == 0 {
		return ""
	}
	// scrub individual keys (fail closed on key-shaped tokens)
	clean := make([]string, 0, len(keys))
	for _, k := range keys {
		k = scrub.Text(k)
		if k == "" || k == "[REDACTED]" || strings.Contains(k, "[REDACTED]") {
			continue
		}
		clean = append(clean, k)
	}
	if len(clean) == 0 {
		return ""
	}
	// OR group of quoted/bare terms
	parts := make([]string, 0, len(clean))
	for _, k := range clean {
		if strings.ContainsAny(k, " \t") {
			parts = append(parts, `"`+k+`"`)
		} else {
			parts = append(parts, k)
		}
	}
	core := "(" + strings.Join(parts, " OR ") + ")"
	q := core + " -is:retweet lang:en"
	if len(q) > 480 {
		// drop keys until it fits
		for len(parts) > 1 && len(q) > 480 {
			parts = parts[:len(parts)-1]
			core = "(" + strings.Join(parts, " OR ") + ")"
			q = core + " -is:retweet lang:en"
		}
	}
	return q
}

// Rank sorts posts by Score descending and returns top n.
func Rank(posts []xsearch.Post, n int) []xsearch.Post {
	if n <= 0 {
		n = 5
	}
	cp := append([]xsearch.Post(nil), posts...)
	sort.SliceStable(cp, func(i, j int) bool {
		if cp[i].Score == cp[j].Score {
			return cp[i].LikeCount > cp[j].LikeCount
		}
		return cp[i].Score > cp[j].Score
	})
	// dedupe by id
	seen := map[string]bool{}
	out := make([]xsearch.Post, 0, n)
	for _, p := range cp {
		if p.ID == "" || seen[p.ID] {
			continue
		}
		seen[p.ID] = true
		out = append(out, p)
		if len(out) >= n {
			break
		}
	}
	return out
}

// PostsToCards maps ranked X posts → plane=research cards (kind=real, generated=false).
func PostsToCards(posts []xsearch.Post, topicKeys []string) []jamclient.Card {
	cards := make([]jamclient.Card, 0, len(posts))
	for _, p := range posts {
		who := p.Username
		if who == "" {
			who = "x"
		}
		title := "@" + who
		body := strings.TrimSpace(p.Text)
		if len([]rune(body)) > 480 {
			body = string([]rune(body)[:477]) + "..."
		}
		cards = append(cards, jamclient.Card{
			Kind:          "real",
			Title:         title,
			Body:          body,
			MediaURL:      p.MediaURL,
			SourceURL:     p.SourceURL,
			Generated:     false,
			Justification: fmt.Sprintf("x search/recent rank=%.1f likes=%d", p.Score, p.LikeCount),
			Plane:         "research",
			TopicKeys:     topicKeys,
		})
	}
	return cards
}

// Run executes the full pipeline:
//
//	session context → distill → query → X recent → rank → digest → research cards
//
// If searcher returns ErrNoBearer, status=hold. Empty matches → hold (no filler).
func Run(in Input, searcher Searcher, digester Digester) Result {
	max := in.MaxCards
	if max <= 0 {
		max = 5
	}

	d := Distill(in.Meta, in.Tips)
	q := BuildQueryFrom(d)
	if q == "" {
		return Result{
			Status:    "skip",
			Reason:    "no topic_keys or context to search",
			Distilled: d,
		}
	}

	posts, err := searcher.SearchRecent(q, 20)
	if err != nil {
		if err == xsearch.ErrNoBearer {
			return Result{Query: q, Status: "hold", Reason: "X_BEARER_TOKEN missing — skip X search", Distilled: d}
		}
		return Result{Query: q, Status: "hold", Reason: "x search error: " + err.Error(), Distilled: d}
	}
	if len(posts) == 0 {
		return Result{Query: q, Status: "hold", Reason: "no recent posts matched", Distilled: d}
	}

	ranked := Rank(posts, max)
	keys := normalizeKeys(d.TopicKeys, 8)
	cards := PostsToCards(ranked, keys)

	// optional digest card (fail-open: errors ignored)
	if digester != nil {
		union := d.Union
		if union == "" {
			union = in.Meta.ContextUnion
		}
		if dc, err := digester.Digest(union, ranked); err == nil && dc != nil {
			cards = append(cards, *dc)
		}
	}

	return Result{
		Query:     q,
		Status:    "ok",
		Reason:    fmt.Sprintf("%d research cards from %d posts", len(cards), len(posts)),
		Cards:     cards,
		Posts:     ranked,
		Distilled: d,
	}
}

var stopwords = map[string]bool{
	"the": true, "and": true, "for": true, "with": true, "from": true,
	"this": true, "that": true, "empty": true, "jam": true, "waiting": true,
	"active": true, "joined": true, "host": true, "member": true,
	"anon": true, "context": true, "mode": true, "main": true, "master": true,
	"fix": true, "feat": true, "files": true, "file": true, "err": true,
	"error": true, "failed": true, "todo": true, "goal": true, "wip": true,
	"src": true, "lib": true, "pkg": true, "bin": true, "test": true,
	"tests": true, "internal": true,
}

func normalizeKeys(keys []string, limit int) []string {
	seen := map[string]bool{}
	out := make([]string, 0, limit)
	for _, k := range keys {
		k = strings.ToLower(strings.TrimSpace(strings.TrimPrefix(k, "#")))
		// drop multi-word down to tokens already split elsewhere; keep phrases ≤3 words
		if strings.ContainsAny(k, " \t") {
			// keep short phrases (e.g. "active view") as quoted later
			parts := strings.Fields(k)
			if len(parts) > 3 {
				k = strings.Join(parts[:3], " ")
			}
		}
		// explicit topic_keys may be short (e.g. "go", "x")
		if k == "" || seen[k] || stopwords[k] || len(k) < 2 {
			continue
		}
		seen[k] = true
		out = append(out, k)
		if len(out) >= limit {
			break
		}
	}
	return out
}

func extractKeys(text string, limit int) []string {
	var keys []string
	for _, tok := range strings.FieldsFunc(text, func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r) && r != '_' && r != '-'
	}) {
		t := strings.ToLower(tok)
		// mined words need ≥3 chars to avoid noise
		if len(t) < 3 || stopwords[t] {
			continue
		}
		keys = append(keys, t)
	}
	return normalizeKeys(keys, limit)
}
