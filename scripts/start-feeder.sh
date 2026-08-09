#!/usr/bin/env bash
# Start Grok Build TUI wired to Feeder.
#
# Usage (from anywhere):
#   ./scripts/start-feeder.sh
#   FEEDER_BASE_URL=https://your.up.railway.app ./scripts/start-feeder.sh
#   ./scripts/start-feeder.sh --release
#
# Env set for this process:
#   FEEDER_BASE_URL   default http://127.0.0.1:8787
#   FEEDER_USER_ID    <device-name>-local  (sanitized hostname)
#   FEEDER_API_KEY    optional, from env or empty
#
# Does not start feeder-service; run that separately (or pass a remote URL).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# --- device name → user id -------------------------------------------------
device_name() {
  local n=""
  if command -v scutil >/dev/null 2>&1; then
    n="$(scutil --get LocalHostName 2>/dev/null || true)"
  fi
  if [[ -z "${n}" ]]; then
    n="$(hostname -s 2>/dev/null || hostname 2>/dev/null || echo device)"
  fi
  # sanitize: alnum and hyphen only, collapse runs, trim
  n="$(printf '%s' "$n" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-+/-/g')"
  if [[ -z "${n}" ]]; then
    n="device"
  fi
  printf '%s' "$n"
}

export FEEDER_BASE_URL="${FEEDER_BASE_URL:-http://127.0.0.1:8787}"
export FEEDER_USER_ID="${FEEDER_USER_ID:-$(device_name)-local}"
# pass through if already set
export FEEDER_API_KEY="${FEEDER_API_KEY:-}"
export FEEDER_URL="${FEEDER_URL:-$FEEDER_BASE_URL}"

RELEASE=0
EXTRA_ARGS=()
for arg in "$@"; do
  case "$arg" in
    --release) RELEASE=1 ;;
    -h|--help)
      cat <<'EOF'
Start Grok Build TUI wired to Feeder.

Usage:
  ./scripts/start-feeder.sh
  FEEDER_BASE_URL=https://your.up.railway.app ./scripts/start-feeder.sh
  ./scripts/start-feeder.sh --release

Exports for this process:
  FEEDER_BASE_URL   default http://127.0.0.1:8787
  FEEDER_USER_ID    <device-name>-local  (sanitized hostname)
  FEEDER_API_KEY    optional (pass through if already set)

Does not start feeder-service — run API separately or point FEEDER_BASE_URL at Railway.
In the TUI, open the feed with /feeder
EOF
      exit 0
      ;;
    *) EXTRA_ARGS+=("$arg") ;;
  esac
done

echo "Feeder TUI"
echo "  FEEDER_BASE_URL=$FEEDER_BASE_URL"
echo "  FEEDER_USER_ID=$FEEDER_USER_ID"
echo "  cwd=$ROOT"
echo "  open with /feeder after launch"
echo

# Quick health hint (non-fatal)
if command -v curl >/dev/null 2>&1; then
  if curl -sf --connect-timeout 1 --max-time 2 "${FEEDER_BASE_URL}/health" >/dev/null 2>&1; then
    echo "  feeder-service: reachable"
  else
    echo "  feeder-service: not reachable at $FEEDER_BASE_URL"
    echo "  (TUI will fall back to fixtures; start API or set FEEDER_BASE_URL)"
  fi
  echo
fi

# macOS bash 3.2 + set -u: empty "${arr[@]}" is an unbound-variable error.
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
