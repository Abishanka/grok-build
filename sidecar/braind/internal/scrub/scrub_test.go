package scrub_test

import (
	"strings"
	"testing"

	"github.com/Abishanka/grok-build/sidecar/braind/internal/scrub"
)

func TestRedactSK(t *testing.T) {
	s := scrub.Text("key sk-abcdefghijklmnopqrstuvwxyz123456 end")
	if strings.Contains(s, "sk-abc") {
		t.Fatalf("sk still present: %q", s)
	}
	if !strings.Contains(s, "[REDACTED]") {
		t.Fatalf("want REDACTED: %q", s)
	}
}

func TestRedactAKIA(t *testing.T) {
	s := scrub.Text("aws AKIAIOSFODNN7EXAMPLE here")
	if strings.Contains(s, "AKIAIOSFODNN7EXAMPLE") {
		t.Fatalf("AKIA still present: %q", s)
	}
}

func TestRedactXAI(t *testing.T) {
	tok := "xai-" + strings.Repeat("a", 24)
	s := scrub.Text("hdr " + tok)
	if strings.Contains(s, tok) {
		t.Fatalf("xai key still present: %q", s)
	}
}

func TestLeaveNormalCode(t *testing.T) {
	in := "fixing ActiveView::Sidecar ratatui chrome"
	if scrub.Text(in) != in {
		t.Fatalf("over-redacted: %q", scrub.Text(in))
	}
}
