package algorithm

import (
	"path/filepath"
	"regexp"
	"strings"
	"unicode"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/jamclient"
	"github.com/Abishanka/grok-build/sidecar/braind/internal/scrub"
)

// Distilled is richer session context for query formation.
// Built from meta + tips: open files, error snippets, goals/chat, topic_keys.
type Distilled struct {
	TopicKeys []string
	Union     string   // scrubbed one-liner for digest / logging
	Files     []string // basenames (e.g. algorithm.go, sidecar.rs)
	Errors    []string // short error-ish phrases
	Goals     []string // chat / goal lines
	Branches  []string
}

var (
	// path-ish tokens: foo/bar.rs, src\main.go, ./pkg/x.go
	rePath = regexp.MustCompile(`(?i)(?:[\w.-]+[/\\])+[\w.-]+\.[a-z0-9]{1,8}`)
	// standalone file with code extension
	reFile = regexp.MustCompile(`(?i)\b[\w.-]+\.(?:go|rs|ts|tsx|js|jsx|py|md|toml|yaml|yml|json|css|html|sh|sql|proto)\b`)
	// error-ish fragments
	reErr = regexp.MustCompile(`(?i)(?:\berror\b|\bpanic\b|\bfailed\b|\bexception\b|\bE[0-9]{2,4}\b|cannot find|undefined|unresolved|borrow|segfault|nil pointer|type mismatch)[^\n.!?]{0,80}`)
	// goal-ish prefixes in chat
	reGoal = regexp.MustCompile(`(?i)^\s*(?:goal|todo|fix|ship|implement|debug|wip)[:\s]+(.+)$`)
)

// Distill coalesces meta + tips into session context and scrubs secrets.
// Fail-open: never errors; empty distill → BuildQuery returns "" → HOLD/skip.
func Distill(meta jamclient.Meta, tips []jamclient.Tip) Distilled {
	d := Distilled{}

	// --- topic keys ---
	keys := append([]string{}, meta.TopicKeys...)
	for _, t := range tips {
		keys = append(keys, t.TopicKeys...)
	}
	// members on meta may carry keys too
	for _, m := range meta.Members {
		keys = append(keys, m.TopicKeys...)
	}

	// --- text bags for extraction ---
	var blobs []string
	if meta.ContextUnion != "" && meta.ContextUnion != "empty jam" {
		blobs = append(blobs, meta.ContextUnion)
	}
	unionParts := make([]string, 0, len(tips)+len(meta.Members)+1)
	if meta.ContextUnion != "" && meta.ContextUnion != "empty jam" {
		unionParts = append(unionParts, meta.ContextUnion)
	}

	collectTip := func(t jamclient.Tip) {
		if t.Context != "" {
			blobs = append(blobs, t.Context)
			bit := t.Origin + ": " + t.Context
			unionParts = append(unionParts, bit)
		}
		if t.Chat != "" {
			blobs = append(blobs, t.Chat)
			d.Goals = append(d.Goals, clampLine(t.Chat, 120))
			if m := reGoal.FindStringSubmatch(t.Chat); len(m) > 1 {
				d.Goals = append(d.Goals, clampLine(m[1], 80))
			}
		}
		if t.DirtySummary != "" {
			blobs = append(blobs, t.DirtySummary)
		}
		if t.Branch != "" {
			d.Branches = append(d.Branches, t.Branch)
			// branch names often encode topic (sidecar-main, fix-x-search)
			keys = append(keys, branchTokens(t.Branch)...)
		}
	}
	for _, t := range tips {
		collectTip(t)
	}
	for _, m := range meta.Members {
		collectTip(m)
	}

	// extract files + errors from all blobs
	for _, b := range blobs {
		d.Files = append(d.Files, extractFiles(b)...)
		d.Errors = append(d.Errors, extractErrors(b)...)
	}

	// file basenames → topic keys (stem without extension when useful)
	for _, f := range d.Files {
		keys = append(keys, fileKey(f))
	}
	// error keywords as light keys
	for _, e := range d.Errors {
		keys = append(keys, extractKeys(e, 3)...)
	}
	// goals contribute keys
	for _, g := range d.Goals {
		keys = append(keys, extractKeys(g, 4)...)
	}

	d.TopicKeys = normalizeKeys(scrub.Strings(keys), 16)
	d.Files = uniqueClamp(d.Files, 8)
	d.Errors = uniqueClamp(scrub.Strings(d.Errors), 4)
	d.Goals = uniqueClamp(scrub.Strings(d.Goals), 4)
	d.Branches = uniqueClamp(d.Branches, 4)

	union := strings.Join(unionParts, " · ")
	if union == "" && len(d.Files) > 0 {
		union = "files: " + strings.Join(d.Files, ", ")
	}
	if len(d.Errors) > 0 {
		union += " · err: " + d.Errors[0]
	}
	if len(d.Goals) > 0 && !strings.Contains(union, d.Goals[0]) {
		union += " · " + d.Goals[0]
	}
	d.Union = scrub.Text(scrub.Clamp(union, 360))

	return d
}

// BuildQueryFrom uses distilled session context (files/errors/goals) for X query.
func BuildQueryFrom(d Distilled) string {
	// Prefer explicit topic keys; BuildQuery also mines union if empty.
	return BuildQuery(d.TopicKeys, d.Union)
}

func extractFiles(text string) []string {
	var out []string
	for _, m := range rePath.FindAllString(text, 12) {
		base := filepath.Base(strings.ReplaceAll(m, "\\", "/"))
		if base != "" && base != "." && base != "/" {
			out = append(out, base)
		}
	}
	for _, m := range reFile.FindAllString(text, 12) {
		out = append(out, m)
	}
	return out
}

func extractErrors(text string) []string {
	var out []string
	for _, m := range reErr.FindAllString(text, 6) {
		m = strings.TrimSpace(m)
		if len(m) < 5 {
			continue
		}
		out = append(out, clampLine(m, 100))
	}
	return out
}

func fileKey(name string) string {
	name = strings.TrimSpace(name)
	base := filepath.Base(name)
	// stem: algorithm.go → algorithm; ActiveView.rs → activeview
	ext := filepath.Ext(base)
	stem := strings.TrimSuffix(base, ext)
	// split CamelCase / snake
	parts := splitIdent(stem)
	if len(parts) == 0 {
		return strings.ToLower(stem)
	}
	// return longest meaningful part first preference handled by normalizeKeys order
	return strings.ToLower(strings.Join(parts, " "))
}

func splitIdent(s string) []string {
	var parts []string
	// snake / kebab
	for _, p := range strings.FieldsFunc(s, func(r rune) bool {
		return r == '_' || r == '-' || r == '.'
	}) {
		// CamelCase split
		var cur strings.Builder
		runes := []rune(p)
		for i, r := range runes {
			if i > 0 && unicode.IsUpper(r) && (unicode.IsLower(runes[i-1]) || (i+1 < len(runes) && unicode.IsLower(runes[i+1]))) {
				if cur.Len() > 0 {
					parts = append(parts, cur.String())
					cur.Reset()
				}
			}
			cur.WriteRune(r)
		}
		if cur.Len() > 0 {
			parts = append(parts, cur.String())
		}
	}
	return parts
}

func branchTokens(branch string) []string {
	branch = strings.TrimPrefix(branch, "refs/heads/")
	var out []string
	for _, p := range strings.FieldsFunc(branch, func(r rune) bool {
		return r == '/' || r == '_' || r == '-'
	}) {
		p = strings.ToLower(p)
		if len(p) >= 3 && !stopwords[p] {
			out = append(out, p)
		}
	}
	return out
}

func clampLine(s string, n int) string {
	s = strings.Join(strings.Fields(s), " ")
	return scrub.Clamp(s, n)
}

func uniqueClamp(in []string, limit int) []string {
	seen := map[string]bool{}
	out := make([]string, 0, limit)
	for _, s := range in {
		s = strings.TrimSpace(s)
		if s == "" {
			continue
		}
		k := strings.ToLower(s)
		if seen[k] {
			continue
		}
		seen[k] = true
		out = append(out, s)
		if len(out) >= limit {
			break
		}
	}
	return out
}
