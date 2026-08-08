// Package worker debounces per-jam tip intake and runs the content algorithm.
package worker

import (
	"log"
	"sync"
	"time"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/algorithm"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/config"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/digest"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/xaisearch"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/xsearch"
)

type Worker struct {
	Cfg      config.Config
	Jam      *jamclient.Client
	X        *xsearch.Client
	Digest   *digest.Client
	XAISearch *xaisearch.Client
	Log      *log.Logger

	mu           sync.Mutex
	lastRun      time.Time
	sinceTS      float64
	pending      bool
	bootstrapped bool // one free run from existing meta when tips cursor is empty
	lastQuery    string
	seenIDs      map[string]bool // posted source urls / post ids
	seenCards    int
	// bgWg tracks optional x_search goroutines (tests can Wait).
	bgWg sync.WaitGroup
}

func New(cfg config.Config) *Worker {
	j := jamclient.New(cfg.JamHost, cfg.JamID, cfg.JamToken)
	x := xsearch.New(cfg.XSearchBaseURL, cfg.XBearerToken)
	var d *digest.Client
	if cfg.XAIAPIKey != "" {
		d = digest.New(cfg.XAIChatURL, cfg.XAIAPIKey, cfg.XAIModel)
	}
	var xs *xaisearch.Client
	if cfg.EnableXAISearch && cfg.XAIAPIKey != "" {
		xs = xaisearch.New(cfg.XAISearchURL, cfg.XAIAPIKey, cfg.XAIModel, cfg.XAISearchTimeout)
	}
	return &Worker{
		Cfg:       cfg,
		Jam:       j,
		X:         x,
		Digest:    d,
		XAISearch: xs,
		Log:       log.Default(),
		seenIDs:   map[string]bool{},
	}
}

// Tick polls jamd once. Debounce: at most one algorithm run per Debounce window.
// Coalesces latest tips; returns true if a run executed.
func (w *Worker) Tick() (bool, error) {
	meta, err := w.Jam.Meta()
	if err != nil {
		return false, err
	}
	tips, err := w.Jam.Tips(w.sinceTS)
	if err != nil {
		return false, err
	}

	w.mu.Lock()
	for _, t := range tips {
		if t.TS > w.sinceTS {
			w.sinceTS = t.TS
		}
		w.pending = true
	}
	// One free wake from existing meta on first tick (jam already has members/tips).
	// Does not re-fire every debounce window — avoids hammering X on HOLD.
	if !w.pending && !w.bootstrapped && (len(meta.TopicKeys) > 0 || meta.ContextUnion != "" && meta.ContextUnion != "empty jam") {
		w.pending = true
		w.bootstrapped = true
	}
	if !w.pending {
		w.mu.Unlock()
		return false, nil
	}
	// debounce
	if !w.lastRun.IsZero() && time.Since(w.lastRun) < w.Cfg.Debounce {
		w.mu.Unlock()
		return false, nil
	}
	w.pending = false
	w.lastRun = time.Now()
	w.mu.Unlock()

	return true, w.run(meta, tips)
}

// RunNow forces one algorithm pass (used by smoke / tests).
func (w *Worker) RunNow() error {
	meta, err := w.Jam.Meta()
	if err != nil {
		return err
	}
	tips, err := w.Jam.Tips(0)
	if err != nil {
		return err
	}
	w.mu.Lock()
	w.lastRun = time.Now()
	w.pending = false
	w.mu.Unlock()
	return w.run(meta, tips)
}

// WaitBackground blocks until optional x_search goroutines finish (tests).
func (w *Worker) WaitBackground() {
	w.bgWg.Wait()
}

func (w *Worker) run(meta jamclient.Meta, tips []jamclient.Tip) error {
	in := algorithm.Input{
		Meta:     meta,
		Tips:     tips,
		MaxCards: w.Cfg.MaxCards,
	}
	var dig algorithm.Digester
	if w.Digest != nil {
		dig = w.Digest
	}
	// Main path: X search/recent (fast). Never blocked by xAI.
	res := algorithm.Run(in, w.X, dig)
	w.Log.Printf("braind jam=%s status=%s query=%q reason=%s cards=%d files=%d errs=%d",
		w.Cfg.JamID, res.Status, res.Query, res.Reason, len(res.Cards),
		len(res.Distilled.Files), len(res.Distilled.Errors))

	w.mu.Lock()
	w.lastQuery = res.Query
	w.mu.Unlock()

	if res.Status == "ok" && len(res.Cards) > 0 {
		if err := w.postFresh(res.Cards); err != nil {
			return err
		}
	}

	// Optional background x_search — does not block main path; fail-open.
	if w.XAISearch != nil && res.Query != "" {
		union := res.Distilled.Union
		if union == "" {
			union = meta.ContextUnion
		}
		q := res.Query
		keys := append([]string{}, res.Distilled.TopicKeys...)
		w.bgWg.Add(1)
		go func() {
			defer w.bgWg.Done()
			w.runXAISearch(union, q, keys)
		}()
	}

	// HOLD / skip without bearer or matches is success (empty beats filler).
	return nil
}

func (w *Worker) runXAISearch(union, query string, topicKeys []string) {
	findings, err := w.XAISearch.Search(union, query)
	if err != nil {
		w.Log.Printf("braind jam=%s x_search hold: %v", w.Cfg.JamID, err)
		return
	}
	if len(findings) == 0 {
		w.Log.Printf("braind jam=%s x_search empty", w.Cfg.JamID)
		return
	}
	cards := xaisearch.ToCards(findings, topicKeys)
	if len(cards) == 0 {
		return
	}
	// Cap secondary cards so they don't flood the feed.
	maxSec := 3
	if w.Cfg.MaxCards > 0 && w.Cfg.MaxCards < maxSec {
		maxSec = w.Cfg.MaxCards
	}
	if len(cards) > maxSec {
		cards = cards[:maxSec]
	}
	if err := w.postFresh(cards); err != nil {
		w.Log.Printf("braind jam=%s x_search post err: %v", w.Cfg.JamID, err)
		return
	}
	w.Log.Printf("braind jam=%s x_search posted %d cards", w.Cfg.JamID, len(cards))
}

func (w *Worker) postFresh(cards []jamclient.Card) error {
	// dedupe against already-posted source urls / title|body
	w.mu.Lock()
	fresh := make([]jamclient.Card, 0, len(cards))
	for _, c := range cards {
		key := c.SourceURL
		if key == "" {
			key = c.Title + "|" + c.Body
		}
		if w.seenIDs[key] {
			continue
		}
		w.seenIDs[key] = true
		fresh = append(fresh, c)
	}
	w.mu.Unlock()

	if len(fresh) == 0 {
		w.Log.Printf("braind jam=%s all cards already seen", w.Cfg.JamID)
		return nil
	}

	posted, err := w.Jam.PostCards(fresh)
	if err != nil {
		return err
	}
	w.mu.Lock()
	w.seenCards += len(posted)
	w.mu.Unlock()
	w.Log.Printf("braind jam=%s posted %d cards", w.Cfg.JamID, len(posted))
	return nil
}

// Loop polls until stop is closed.
func (w *Worker) Loop(stop <-chan struct{}) {
	w.Log.Printf("braind loop jam=%s host=%s debounce=%s poll=%s x_bearer=%v xai=%v x_search=%v",
		w.Cfg.JamID, w.Cfg.JamHost, w.Cfg.Debounce, w.Cfg.PollInterval,
		w.Cfg.XBearerToken != "", w.Cfg.XAIAPIKey != "", w.XAISearch != nil)
	t := time.NewTicker(w.Cfg.PollInterval)
	defer t.Stop()
	// immediate first tick
	if _, err := w.Tick(); err != nil {
		w.Log.Printf("braind tick err: %v", err)
	}
	for {
		select {
		case <-stop:
			w.WaitBackground()
			return
		case <-t.C:
			if _, err := w.Tick(); err != nil {
				w.Log.Printf("braind tick err: %v", err)
			}
		}
	}
}
