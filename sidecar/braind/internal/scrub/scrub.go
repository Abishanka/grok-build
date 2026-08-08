// Package scrub redacts secrets from text leaving braind toward external APIs.
// Mirrors jamd/internal/filter patterns — keep lightweight, fail closed on matches.
package scrub

import (
	"math"
	"regexp"
	"strings"
	"unicode/utf8"
)

var patterns = []*regexp.Regexp{
	regexp.MustCompile(`sk-[A-Za-z0-9_\-]{10,}`),
	regexp.MustCompile(`sk_live_[A-Za-z0-9_\-]{10,}`),
	regexp.MustCompile(`AKIA[0-9A-Z]{16}`),
	regexp.MustCompile(`xai-[A-Za-z0-9_\-]{20,}`),
	regexp.MustCompile(`ghp_[A-Za-z0-9]{20,}`),
	regexp.MustCompile(`github_pat_[A-Za-z0-9_]{20,}`),
	regexp.MustCompile(`Bearer\s+[A-Za-z0-9_\-\.=]+`),
}

var hiEntropy = regexp.MustCompile(`\b[A-Za-z0-9_\-+/=]{32,}\b`)

const redacted = "[REDACTED]"

// Text redacts key-shaped and high-entropy tokens from s.
func Text(s string) string {
	if s == "" {
		return s
	}
	out := s
	for _, p := range patterns {
		out = p.ReplaceAllString(out, redacted)
	}
	out = hiEntropy.ReplaceAllStringFunc(out, func(tok string) string {
		if entropy(tok) >= 3.5 {
			return redacted
		}
		return tok
	})
	return out
}

// Strings maps Text over a slice.
func Strings(in []string) []string {
	if len(in) == 0 {
		return in
	}
	out := make([]string, len(in))
	for i, s := range in {
		out[i] = Text(s)
	}
	return out
}

// Clamp trims and bounds rune length.
func Clamp(s string, max int) string {
	s = strings.TrimSpace(s)
	if max <= 0 || utf8.RuneCountInString(s) <= max {
		return s
	}
	r := []rune(s)
	return string(r[:max])
}

func entropy(s string) float64 {
	if len(s) < 32 {
		return 0
	}
	freq := map[rune]float64{}
	n := 0.0
	for _, r := range s {
		freq[r]++
		n++
	}
	var h float64
	for _, c := range freq {
		p := c / n
		h -= p * math.Log2(p)
	}
	return h
}
