#!/usr/bin/env bash
# Start Grok Build TUI wired to the live Feeder API (Railway by default).
#
# Usage:
#   ./scripts/start-feeder.sh
#   ./scripts/start-feeder.sh --local          # http://127.0.0.1:8787
#   FEEDER_BASE_URL=https://… ./scripts/start-feeder.sh
#   ./scripts/start-feeder.sh --release
#
# Env:
#   FEEDER_BASE_URL   default https://feeder-api-production.up.railway.app
#   FEEDER_USER_ID    <device-name>-local
#   FEEDER_API_KEY    optional

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RAILWAY_DEFAULT="https://feeder-api-production.up.railway.app"
LOCAL_DEFAULT="http://127.0.0.1:8787"

device_name() {
  local n=""
  if command -v scutil >/dev/null 2>&1; then
    n="$(scutil --get LocalHostName 2>/dev/null || true)"
  fi
  if [[ -z "${n}" ]]; then
    n="$(hostname -s 2>/dev/null || hostname 2>/dev/null || echo device)"
  fi
  n="$(printf '%s' "$n" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-+/-/g')"
  if [[ -z "${n}" ]]; then
    n="device"
  fi
  printf '%s' "$n"
}

RELEASE=0
USE_LOCAL=0
EXTRA_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE=1 ;;
    --local) USE_LOCAL=1 ;;
    -h|--help)
      cat <<'EOF'
Start Grok Build TUI wired to Feeder (Railway production API by default).

Usage:
  ./scripts/start-feeder.sh              # Railway production API
  ./scripts/start-feeder.sh --local      # http://127.0.0.1:8787
  FEEDER_BASE_URL=https://… ./scripts/start-feeder.sh
  ./scripts/start-feeder.sh --release

In TUI:
  open a coding session, then /feeder
  Dock opens FOCUSED on the feed
  j / k          next / prev post
  u / Enter      use post as agent context
  e              explain
  x              dismiss
  o              open on X
  r              refresh
  Esc            focus agent (dock stays open)
  Ctrl+F / Tab   toggle focus Feeder ↔ Agent
  q              close dock
EOF
      exit 0
      ;;
    *) EXTRA_ARGS+=("$arg") ;;
  esac
done

if [[ -z "${FEEDER_BASE_URL:-}" ]]; then
  if [[ "$USE_LOCAL" -eq 1 ]]; then
    export FEEDER_BASE_URL="$LOCAL_DEFAULT"
  else
    export FEEDER_BASE_URL="$RAILWAY_DEFAULT"
  fi
fi
export FEEDER_USER_ID="${FEEDER_USER_ID:-$(device_name)-local}"
export FEEDER_API_KEY="${FEEDER_API_KEY:-}"
export FEEDER_URL="${FEEDER_URL:-$FEEDER_BASE_URL}"
# Ensure the Rust client sees the same URL even if something unsets FEEDER_BASE_URL later
export GROK_FEEDER="${GROK_FEEDER:-1}"

echo "Feeder TUI"
echo "  FEEDER_BASE_URL=$FEEDER_BASE_URL"
echo "  FEEDER_USER_ID=$FEEDER_USER_ID"
echo "  cwd=$ROOT"
echo "  /feeder opens dock focused · j/k · u use · Esc agent · q close"
echo

if command -v curl >/dev/null 2>&1; then
  if body="$(curl -sf --connect-timeout 3 --max-time 8 "${FEEDER_BASE_URL}/health" 2>/dev/null)"; then
    echo "  feeder-service: reachable  $body"
  else
    echo "  feeder-service: NOT reachable at $FEEDER_BASE_URL"
    echo "  (dock shows fixtures, then retries in background)"
  fi
  # Warm the catalog so the first /feeder open is fast
  if curl -sf --connect-timeout 3 --max-time 12 \
      -X POST "${FEEDER_BASE_URL}/v1/feed/query" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: ${FEEDER_USER_ID}" \
      -d '{"user_id":"'"${FEEDER_USER_ID}"'","limit":5,"context":{"recent_prompts":["rust oauth","coding agent"]}}' \
      >/dev/null 2>&1; then
    echo "  feeder-service: feed query ok (catalog warm)"
  else
    echo "  feeder-service: feed query slow/failed (dock will retry)"
  fi
  echo
fi

run_pager() {
  local -a cmd=(cargo run -p xai-grok-pager-bin)
  if [[ "$RELEASE" -eq 1 ]]; then
    cmd+=(--release)
  fi
  if [[ "${#EXTRA_ARGS[@]}" -gt 0 ]]; then
    cmd+=(-- "${EXTRA_ARGS[@]}")
  fi
  exec "${cmd[@]}"
}

run_pager
