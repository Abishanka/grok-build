#!/usr/bin/env bash
# Public URL for the feed page + brain. Requires: brew install cloudflared
# Serves web/ + proxies /cards etc. from the brain via the brain itself?
# Simplest: brain serves the static page too — until then, tunnel the brain:
exec cloudflared tunnel --url http://localhost:${SIDECAR_PORT:-7717}
