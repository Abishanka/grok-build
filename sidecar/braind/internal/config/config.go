// Package config loads braind runtime settings from the environment.
package config

import (
	"os"
	"strconv"
	"strings"
	"time"
)

type Config struct {
	JamHost        string
	JamID          string
	JamToken       string
	XBearerToken   string
	XAIAPIKey      string
	XAIBaseURL     string
	XAIModel       string
	PollInterval   time.Duration
	Debounce       time.Duration
	MaxCards       int
	XSearchBaseURL string // override for tests
	XAIChatURL     string // override for tests
	// Optional background xAI x_search path (Responses API).
	EnableXAISearch   bool
	XAISearchURL      string // full /responses URL; default XAIBaseURL+/responses
	XAISearchTimeout  time.Duration
}

func FromEnv() Config {
	c := Config{
		JamHost:          strings.TrimRight(env("JAM_HOST", "http://127.0.0.1:7720"), "/"),
		JamID:            env("JAM_ID", "demo"),
		JamToken:         os.Getenv("JAM_TOKEN"),
		XBearerToken:     firstNonEmpty(os.Getenv("X_BEARER_TOKEN"), os.Getenv("X_BEARER")),
		XAIAPIKey:        os.Getenv("XAI_API_KEY"),
		XAIBaseURL:       strings.TrimRight(env("XAI_BASE_URL", "https://api.x.ai/v1"), "/"),
		XAIModel:         env("XAI_MODEL", "grok-4.5"),
		PollInterval:     durationEnv("BRAIN_POLL_INTERVAL", 3*time.Second),
		Debounce:         durationEnv("BRAIN_DEBOUNCE", 10*time.Second),
		MaxCards:         intEnv("BRAIN_MAX_CARDS", 5),
		XSearchBaseURL:   env("X_API_BASE", "https://api.x.com/2"),
		EnableXAISearch:  boolEnv("BRAIN_ENABLE_XAI_SEARCH", false),
		XAISearchTimeout: durationEnv("BRAIN_XAI_SEARCH_TIMEOUT", 25*time.Second),
	}
	c.XAIChatURL = c.XAIBaseURL + "/chat/completions"
	c.XAISearchURL = env("XAI_RESPONSES_URL", c.XAIBaseURL+"/responses")
	return c
}

func env(k, def string) string {
	if v := strings.TrimSpace(os.Getenv(k)); v != "" {
		return v
	}
	return def
}

func firstNonEmpty(a, b string) string {
	if strings.TrimSpace(a) != "" {
		return strings.TrimSpace(a)
	}
	return strings.TrimSpace(b)
}

func durationEnv(k string, def time.Duration) time.Duration {
	v := strings.TrimSpace(os.Getenv(k))
	if v == "" {
		return def
	}
	if d, err := time.ParseDuration(v); err == nil {
		return d
	}
	// bare seconds
	if n, err := strconv.Atoi(v); err == nil && n > 0 {
		return time.Duration(n) * time.Second
	}
	return def
}

func intEnv(k string, def int) int {
	v := strings.TrimSpace(os.Getenv(k))
	if v == "" {
		return def
	}
	n, err := strconv.Atoi(v)
	if err != nil || n <= 0 {
		return def
	}
	return n
}

func boolEnv(k string, def bool) bool {
	v := strings.TrimSpace(strings.ToLower(os.Getenv(k)))
	if v == "" {
		return def
	}
	switch v {
	case "1", "true", "yes", "on":
		return true
	case "0", "false", "no", "off":
		return false
	default:
		return def
	}
}
