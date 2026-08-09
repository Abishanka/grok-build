#!/usr/bin/env bash
# Start Grok Build TUI on the jam multiplayer branch, pointed at live feeder.
#
# Branch:  sidecar-main  (jam Feeder dock + /feeder jam …)
# Backend: https://feeder-api-production.up.railway.app  (default)
#
# Usage:
#   ./scripts/start-jam-tui.sh                 # host-ish defaults, production feeder
#   ./scripts/start-jam-tui.sh --origin abi     # peer identity
#   ./scripts/start-jam-tui.sh --local          # feeder on :8787
#   ./scripts/start-jam-tui.sh --release        # cargo --release
#   ./scripts/start-jam-tui.sh --no-checkout    # don't touch git branch
#   ./scripts/start-jam-tui.sh --join 'URL'     # print join hint after launch env
#   JAM_ORIGIN=alice ./scripts/start-jam-tui.sh
#
# In the TUI:
#   /feeder
#   /feeder jam start <title>          # host
#   /feeder jam join '<join_url>'      # peer
#   /feeder jam invite | status | leave | end
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BRANCH="${JAM_GIT_BRANCH:-sidecar-main}"
RAILWAY_DEFAULT="https://feeder-api-production.up.railway.app"
LOCAL_DEFAULT="http://127.0.0.1:8787"

RELEASE=0
USE_LOCAL=0
NO_CHECKOUT=0
ORIGIN=""
JOIN_URL=""
EXTRA_ARGS=()

usage() {
  sed -n '2,25p' "$0" | sed 's/^# \?//'
  exit 0
}

device_name() {
  local n=""
  if command -v scutil >/dev/null 2>&1; then
    n="$(scutil --get LocalHostName 2>/dev/null || true)"
  fi
  if [[ -z "${n}" ]]; then
    n="$(hostname -s 2>/dev/null || hostname 2>/dev/null || echo device)"
  fi
  n="$(printf '%s' "$n" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-+/-/g')"
  [[ -n "$n" ]] || n="device"
  printf '%s' "$n"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage ;;
    --release) RELEASE=1; shift ;;
    --local) USE_LOCAL=1; shift ;;
    --no-checkout) NO_CHECKOUT=1; shift ;;
    --origin)
      ORIGIN="${2:-}"; shift 2
      if [[ -z "$ORIGIN" ]]; then echo "error: --origin needs a value" >&2; exit 1; fi
      ;;
    --origin=*) ORIGIN="${1#*=}"; shift ;;
    --join)
      JOIN_URL="${2:-}"; shift 2
      if [[ -z "$JOIN_URL" ]]; then echo "error: --join needs a URL" >&2; exit 1; fi
      ;;
    --join=*) JOIN_URL="${1#*=}"; shift ;;
    --branch)
      BRANCH="${2:-}"; shift 2
      if [[ -z "$BRANCH" ]]; then echo "error: --branch needs a value" >&2; exit 1; fi
      ;;
    --) shift; EXTRA_ARGS+=("$@"); break ;;
    -*)
      echo "unknown flag: $1" >&2
      exit 1
      ;;
    *) EXTRA_ARGS+=("$1"); shift ;;
  esac
done

# ── git: right branch ─────────────────────────────────────────────
if [[ "$NO_CHECKOUT" -eq 0 ]]; then
  if ! command -v git >/dev/null 2>&1; then
    echo "error: git required" >&2
    exit 1
  fi
  if [[ ! -d .git ]]; then
    echo "error: $ROOT is not a git checkout of grok-build" >&2
    exit 1
  fi

  current="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
  if [[ "$current" != "$BRANCH" ]]; then
    echo "→ switching branch: $current → $BRANCH"
    # Keep local jam work; only switch if clean enough or user already on worktree
    if git show-ref --verify --quiet "refs/heads/$BRANCH"; then
      git checkout "$BRANCH"
    elif git show-ref --verify --quiet "refs/remotes/origin/$BRANCH"; then
      git checkout -B "$BRANCH" "origin/$BRANCH"
    else
      echo "error: branch '$BRANCH' not found locally or on origin" >&2
      echo "  available: $(git branch --list | tr '\n' ' ')" >&2
      exit 1
    fi
  else
    echo "→ already on $BRANCH"
  fi

  # Jam multiplayer lives in working tree on sidecar-main (may be uncommitted).
  if ! git grep -q "FeederJamCommand" -- 'crates/codegen/xai-grok-pager/src' 2>/dev/null \
    && ! grep -rq "FeederJamCommand" crates/codegen/xai-grok-pager/src 2>/dev/null; then
    echo "warn: FeederJamCommand not found — jam slash cmds may be missing on this revision" >&2
    echo "  expected jam work on sidecar-main (possibly uncommitted)." >&2
  else
    echo "→ jam TUI symbols present (FeederJamCommand)"
  fi
else
  echo "→ --no-checkout: using $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
fi

# ── load local secrets (.env) for Grok agent keys ─────────────────
# Feeder hub keys live on Railway. The TUI still needs XAI_API_KEY (or
# grok login / auth.json) for the coding agent itself.
if [[ -f "$ROOT/scripts/env.sh" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT/scripts/env.sh" || true
elif [[ -f "$ROOT/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/.env"
  set +a
fi

# ── env: live feeder ──────────────────────────────────────────────
if [[ -z "${FEEDER_BASE_URL:-}" ]]; then
  if [[ "$USE_LOCAL" -eq 1 ]]; then
    export FEEDER_BASE_URL="$LOCAL_DEFAULT"
  else
    export FEEDER_BASE_URL="$RAILWAY_DEFAULT"
  fi
fi
export FEEDER_URL="${FEEDER_URL:-$FEEDER_BASE_URL}"
export GROK_FEEDER="${GROK_FEEDER:-1}"

if [[ -n "$ORIGIN" ]]; then
  export JAM_ORIGIN="$ORIGIN"
  export FEEDER_USER_ID="${FEEDER_USER_ID:-$ORIGIN}"
else
  export JAM_ORIGIN="${JAM_ORIGIN:-$(device_name)}"
  export FEEDER_USER_ID="${FEEDER_USER_ID:-${JAM_ORIGIN}}"
fi

echo ""
echo "Jam TUI launch"
echo "  branch:          $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo n/a) @ $(git rev-parse --short HEAD 2>/dev/null || echo n/a)"
echo "  FEEDER_BASE_URL: $FEEDER_BASE_URL"
echo "  FEEDER_USER_ID:  $FEEDER_USER_ID"
echo "  JAM_ORIGIN:      $JAM_ORIGIN"
echo "  GROK_FEEDER:     $GROK_FEEDER"
if [[ -n "${XAI_API_KEY:-}" ]]; then
  echo "  XAI_API_KEY:     set (${#XAI_API_KEY} chars)"
elif [[ -f "${HOME}/.grok/auth.json" ]] || [[ -f "${GROK_HOME:-$HOME/.grok}/auth.json" ]]; then
  echo "  XAI_API_KEY:     unset (will try grok login / auth.json)"
else
  echo "  XAI_API_KEY:     UNSET — agent won't auth. Add to .env or: grok login"
  echo "                   cp .env.example .env && fill XAI_API_KEY, or source scripts/env.sh"
fi
if [[ -n "${FEEDER_API_KEY:-}" ]]; then
  echo "  FEEDER_API_KEY:  set"
else
  echo "  FEEDER_API_KEY:  unset (ok if Railway feeder has no FEEDER_API_KEY)"
fi
if [[ -n "$JOIN_URL" ]]; then
  echo "  join hint:       /feeder jam join '$JOIN_URL'"
fi
echo ""
echo "How you know it's working:"
echo "  1) /feeder header shows live (not off) after a few seconds"
echo "  2) /feeder jam start → toast with https join URL + JAM· badge"
echo "  3) peer joins → plane strip shows two origins"
echo "  4) you run a tool turn → peer sees your plane status tool:… + feed cards"
echo "  5) curl \$FEEDER_BASE_URL/health | grep jams"
echo ""

# ── health ────────────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
  if body="$(curl -sf --connect-timeout 3 --max-time 8 "${FEEDER_BASE_URL}/health" 2>/dev/null)"; then
    echo "  feeder: reachable  $body"
    if ! printf '%s' "$body" | grep -q '"jams"[[:space:]]*:[[:space:]]*true'; then
      echo "  warn: health missing jams:true — production may not have jam routes" >&2
    fi
  else
    echo "  warn: feeder NOT reachable at $FEEDER_BASE_URL" >&2
  fi
  echo ""
fi

echo "In TUI:"
echo "  /feeder"
echo "  host:  /feeder jam start <title>"
echo "  peer:  /feeder jam join '<join_url>'"
echo "  both:  /feeder jam status | invite | leave | end"
echo ""

# ── run pager ─────────────────────────────────────────────────────
CMD=(cargo run -p xai-grok-pager-bin)
if [[ "$RELEASE" -eq 1 ]]; then
  CMD+=(--release)
fi
if [[ "${#EXTRA_ARGS[@]}" -gt 0 ]]; then
  CMD+=(-- "${EXTRA_ARGS[@]}")
fi

echo "→ ${CMD[*]}"
echo ""
exec "${CMD[@]}"
