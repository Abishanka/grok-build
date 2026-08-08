package algorithm_test

import (
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/algorithm"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/xsearch"
)

type fakeSearch struct {
	posts []xsearch.Post
	err   error
	query string
}

func (f *fakeSearch) SearchRecent(query string, maxResults int) ([]xsearch.Post, error) {
	f.query = query
	if f.err != nil {
		return nil, f.err
	}
	return f.posts, nil
}

func TestBuildQuery(t *testing.T) {
	q := algorithm.BuildQuery([]string{"ActiveView", "E0308", "ratatui"}, "fixing ActiveView")
	if !strings.Contains(q, "activeview") && !strings.Contains(q, "ActiveView") {
		// keys lowercased
		if !strings.Contains(q, "activeview") {
			t.Fatalf("query missing key: %q", q)
		}
	}
	if !strings.Contains(q, "-is:retweet") {
		t.Fatalf("want -is:retweet in %q", q)
	}
	if !strings.Contains(q, "lang:en") {
		t.Fatalf("want lang:en in %q", q)
	}
	if q == "" {
		t.Fatal("empty query")
	}
}

func TestBuildQueryFromContextOnly(t *testing.T) {
	q := algorithm.BuildQuery(nil, "debugging websocket reconnect storms")
	if q == "" {
		t.Fatal("expected extracted keys")
	}
	if !strings.Contains(strings.ToLower(q), "websocket") && !strings.Contains(strings.ToLower(q), "reconnect") {
		t.Fatalf("query: %q", q)
	}
}

func TestRankAndCards(t *testing.T) {
	now := time.Now()
	posts := []xsearch.Post{
		{ID: "1", Text: "low", Username: "a", LikeCount: 1, CreatedAt: now, SourceURL: "https://x.com/a/status/1"},
		{ID: "2", Text: "high", Username: "b", LikeCount: 100, RTCount: 20, CreatedAt: now, SourceURL: "https://x.com/b/status/2"},
		{ID: "2", Text: "dup", Username: "b", LikeCount: 100, CreatedAt: now}, // dup id
	}
	for i := range posts {
		posts[i].Score = xsearch.Score(posts[i])
	}
	ranked := algorithm.Rank(posts, 5)
	if len(ranked) != 2 {
		t.Fatalf("dedupe rank len %d", len(ranked))
	}
	if ranked[0].ID != "2" {
		t.Fatalf("want high first got %+v", ranked[0])
	}
	cards := algorithm.PostsToCards(ranked, []string{"go"})
	if len(cards) != 2 {
		t.Fatalf("cards %d", len(cards))
	}
	c := cards[0]
	if c.Plane != "research" || c.Kind != "real" || c.Generated {
		t.Fatalf("card fields: %+v", c)
	}
	if c.SourceURL == "" {
		t.Fatal("missing source_url")
	}
	if !strings.HasPrefix(c.Title, "@") {
		t.Fatalf("title: %q", c.Title)
	}
}

func TestRunHoldNoBearer(t *testing.T) {
	fs := &fakeSearch{err: xsearch.ErrNoBearer}
	res := algorithm.Run(algorithm.Input{
		Meta: jamclient.Meta{TopicKeys: []string{"rust"}, ContextUnion: "abi: fixing borrow"},
		MaxCards: 3,
	}, fs, nil)
	if res.Status != "hold" {
		t.Fatalf("status %s reason %s", res.Status, res.Reason)
	}
	if res.Query == "" {
		t.Fatal("query should still be built")
	}
	if len(res.Cards) != 0 {
		t.Fatal("no cards on hold")
	}
}

func TestRunOK(t *testing.T) {
	now := time.Now()
	fs := &fakeSearch{posts: []xsearch.Post{
		{
			ID: "99", Text: "ratatui tip of the day", Username: "tui_dev",
			LikeCount: 42, RTCount: 5, CreatedAt: now,
			SourceURL: "https://x.com/tui_dev/status/99",
		},
	}}
	fs.posts[0].Score = xsearch.Score(fs.posts[0])
	res := algorithm.Run(algorithm.Input{
		Meta: jamclient.Meta{
			TopicKeys:    []string{"ratatui", "tui"},
			ContextUnion: "host[active]: polishing sidebar",
		},
		Tips: []jamclient.Tip{
			{Origin: "host", Context: "polishing sidebar", TopicKeys: []string{"ratatui"}, TS: 1},
		},
		MaxCards: 3,
	}, fs, nil)
	if res.Status != "ok" {
		t.Fatalf("status=%s reason=%s", res.Status, res.Reason)
	}
	if len(res.Cards) != 1 {
		t.Fatalf("cards %d", len(res.Cards))
	}
	if res.Cards[0].Plane != "research" || res.Cards[0].Generated {
		t.Fatalf("%+v", res.Cards[0])
	}
	if !strings.Contains(fs.query, "ratatui") {
		t.Fatalf("search query: %q", fs.query)
	}
}

func TestRunSkipEmpty(t *testing.T) {
	fs := &fakeSearch{}
	res := algorithm.Run(algorithm.Input{Meta: jamclient.Meta{ContextUnion: "empty jam"}}, fs, nil)
	if res.Status != "skip" {
		t.Fatalf("want skip got %s (%s)", res.Status, res.Reason)
	}
}

func TestRunSearchError(t *testing.T) {
	fs := &fakeSearch{err: errors.New("429 rate")}
	res := algorithm.Run(algorithm.Input{
		Meta: jamclient.Meta{TopicKeys: []string{"go"}},
	}, fs, nil)
	if res.Status != "hold" {
		t.Fatalf("want hold got %s", res.Status)
	}
}

func TestDistillFromFilesErrorsGoals(t *testing.T) {
	d := algorithm.Distill(jamclient.Meta{
		ContextUnion: "host[active]: touching sidecar/braind/internal/algorithm/algorithm.go",
	}, []jamclient.Tip{
		{
			Origin:       "abi",
			Context:      "error: cannot find ActiveView in scope while editing src/view/sidecar.rs",
			Chat:         "goal: ship freeze badge",
			DirtySummary: "3 files +40/-12 src/view/sidecar.rs",
			Branch:       "fix/sidecar-freeze",
			TopicKeys:    []string{"sidecar"},
		},
	})
	if len(d.Files) == 0 {
		t.Fatalf("expected files, got %#v", d)
	}
	foundAlgo := false
	foundSidecar := false
	for _, f := range d.Files {
		if strings.Contains(strings.ToLower(f), "algorithm.go") {
			foundAlgo = true
		}
		if strings.Contains(strings.ToLower(f), "sidecar.rs") {
			foundSidecar = true
		}
	}
	if !foundAlgo && !foundSidecar {
		t.Fatalf("files missing expected basenames: %#v", d.Files)
	}
	if len(d.Errors) == 0 {
		t.Fatalf("expected error snippets: %#v", d)
	}
	if len(d.Goals) == 0 {
		t.Fatalf("expected goals from chat: %#v", d)
	}
	q := algorithm.BuildQueryFrom(d)
	if q == "" {
		t.Fatal("empty query from rich distill")
	}
	if !strings.Contains(q, "-is:retweet") {
		t.Fatalf("query: %q", q)
	}
	// should pick up sidecar and/or freeze / activeview-ish terms
	low := strings.ToLower(q)
	if !strings.Contains(low, "sidecar") && !strings.Contains(low, "freeze") && !strings.Contains(low, "activeview") {
		t.Fatalf("rich query missing session terms: %q (keys=%v)", q, d.TopicKeys)
	}
}

func TestBuildQueryScrubsSecrets(t *testing.T) {
	secret := "sk-" + strings.Repeat("Z", 24)
	q := algorithm.BuildQuery([]string{secret, "ratatui"}, "leaked "+secret)
	if strings.Contains(q, secret) {
		t.Fatalf("secret in query: %q", q)
	}
	if !strings.Contains(strings.ToLower(q), "ratatui") {
		t.Fatalf("lost good key: %q", q)
	}
}

func TestDistillScrubsUnion(t *testing.T) {
	secret := "sk-" + strings.Repeat("a", 20)
	d := algorithm.Distill(jamclient.Meta{}, []jamclient.Tip{
		{Origin: "h", Context: "debugging with " + secret + " in env"},
	})
	if strings.Contains(d.Union, secret) {
		t.Fatalf("union leaked secret: %q", d.Union)
	}
}

func TestRunUsesTipFilesWhenMetaSparse(t *testing.T) {
	now := time.Now()
	fs := &fakeSearch{posts: []xsearch.Post{
		{
			ID: "7", Text: "rust borrow checker tips", Username: "r",
			LikeCount: 9, CreatedAt: now, SourceURL: "https://x.com/r/status/7",
		},
	}}
	fs.posts[0].Score = xsearch.Score(fs.posts[0])
	res := algorithm.Run(algorithm.Input{
		Meta: jamclient.Meta{ContextUnion: "empty jam"},
		Tips: []jamclient.Tip{
			{
				Origin:  "dev",
				Context: "error: borrow of moved value in crates/tui/src/app.rs",
				Chat:    "fix the freeze race",
			},
		},
		MaxCards: 2,
	}, fs, nil)
	if res.Status != "ok" {
		t.Fatalf("status=%s reason=%s query=%q distilled=%+v", res.Status, res.Reason, res.Query, res.Distilled)
	}
	if res.Query == "" {
		t.Fatal("expected query from tip distill")
	}
	if len(res.Cards) != 1 || res.Cards[0].Plane != "research" {
		t.Fatalf("cards %#v", res.Cards)
	}
	if res.Cards[0].SourceURL == "" {
		t.Fatal("missing source_url")
	}
}
