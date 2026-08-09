#!/usr/bin/env bash
# Populate .env for the Feeder service (grokathon) and the TUI client (grok-build).
#
# Idempotent: existing non-empty values are kept. Missing secrets are prompted
# for when running on a TTY, otherwise left blank and reported at the end.
# Secret values are never echoed — only present/missing status.
#
# Usage:
#   ./scripts/setup-feeder-env.sh
#   ./scripts/setup-feeder-env.sh --non-interactive   # no prompts, report gaps
#   ./scripts/setup-feeder-env.sh --show              # report status, write nothing
#
# Env overrides:
#   SERVICE_DIR     default ../grokathon
#   CLIENT_DIR      default this repo
#   FEEDER_BASE_URL default https://feeder-api-production.up.railway.app
#
# Compatible with bash 3.2 (macOS system bash) — no associative arrays.

set -euo pipefail

CLIENT_DIR="${CLIENT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SERVICE_DIR="${SERVICE_DIR:-$(cd "$CLIENT_DIR/.." && pwd)/grokathon}"
API_DEFAULT="${FEEDER_BASE_URL:-https://feeder-api-production.up.railway.app}"

INTERACTIVE=1
DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    --non-interactive) INTERACTIVE=0 ;;
    --show) DRY_RUN=1; INTERACTIVE=0 ;;
    -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done
[ -t 0 ] || INTERACTIVE=0

bold() { printf '\033[1m%s\033[0m\n' "$1"; }
ok()   { printf '  \033[32m/\033[0m %s\n' "$1"; }
miss() { printf '  \033[31mx\033[0m %s  \033[2m%s\033[0m\n' "$1" "${2:-}"; }
note() { printf '  \033[2m%s\033[0m\n' "$1"; }

# ── key/value store (bash 3.2: prefixed vars + an ordered key list) ───────────
# Keys are validated to [A-Za-z_][A-Za-z0-9_]* so `V_$key` is always a safe name.
ORDER=""          # newline-separated key list, preserves file order
MISSING=""        # newline-separated

valid_key() { case "$1" in [A-Za-z_]*) [ -z "$(printf '%s' "$1" | tr -d 'A-Za-z0-9_')" ] ;; *) return 1 ;; esac; }

reset_file() {
  local k
  for k in $ORDER; do unset "V_$k" 2>/dev/null || true; done
  ORDER=""
}

key_known() { case "
$ORDER" in *"
$1
"*) return 0 ;; esac; return 1; }

put() { # put KEY VALUE — set unconditionally
  valid_key "$1" || return 0
  key_known "$1" || ORDER="$ORDER
$1"
  eval "V_$1=\$2"
}

getv() { eval "printf '%s' \"\${V_$1:-}\""; }
have() { [ -n "$(getv "$1")" ]; }

default() { have "$1" || put "$1" "$2"; }   # set only if empty

secret() { # secret KEY PROMPT [optional]
  local k="$1" prompt="$2" req="${3:-required}" v="" shellval=""
  if have "$k"; then ok "$k"; return 0; fi
  eval "shellval=\${$k:-}"
  if [ -n "$shellval" ]; then put "$k" "$shellval"; ok "$k  (from shell env)"; return 0; fi
  if [ "$INTERACTIVE" = 1 ]; then
    printf '  %s\n' "$prompt"
    printf '    %s (blank to skip): ' "$k"
    read -rs v; echo
    if [ -n "$v" ]; then put "$k" "$v"; ok "$k"; return 0; fi
  fi
  put "$k" ""
  if [ "$req" = "required" ]; then
    miss "$k" "$prompt"; MISSING="$MISSING
$k"
  else
    note "$k  (optional, unset)"
  fi
}

plain() { # plain KEY PROMPT DEFAULT — non-secret, echoed
  local k="$1" def="${3:-}" v=""
  if have "$k"; then ok "$k=$(getv "$k")"; return 0; fi
  if [ "$INTERACTIVE" = 1 ]; then
    printf '    %s [%s]: ' "$k" "$def"
    read -r v
  fi
  put "$k" "${v:-$def}"
  ok "$k=$(getv "$k")"
}

load_env() {
  local f="$1" line key val
  [ -f "$f" ] || return 0
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      \#*|"") continue ;;
      *=*) ;;
      *) continue ;;
    esac
    line="${line# }"; line="${line#export }"
    key="${line%%=*}"; val="${line#*=}"
    key="$(printf '%s' "$key" | tr -d '[:space:]')"
    valid_key "$key" || continue
    # strip one layer of surrounding quotes
    case "$val" in
      \"*\") val="${val#\"}"; val="${val%\"}" ;;
      \'*\') val="${val#\'}"; val="${val%\'}" ;;
    esac
    put "$key" "$val"
  done < "$f"
}

write_env() {
  local f="$1" k tmp
  if [ "$DRY_RUN" = 1 ]; then note "(--show) not writing $f"; return 0; fi
  mkdir -p "$(dirname "$f")"
  [ -f "$f" ] && cp "$f" "$f.bak.$(date +%Y%m%d%H%M%S)"
  tmp="$(mktemp)"
  {
    echo "# Managed by scripts/setup-feeder-env.sh - never commit."
    echo "# Written $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo
    for k in $ORDER; do printf '%s=%s\n' "$k" "$(getv "$k")"; done
  } > "$tmp"
  mv "$tmp" "$f"
  chmod 600 "$f"
  ok "wrote $f (mode 600)"
}

# ══ SERVICE: grokathon/.env ═══════════════════════════════════════════════════
bold ""
bold "Feeder service  ->  $SERVICE_DIR/.env"
if [ ! -d "$SERVICE_DIR" ]; then
  echo "  service dir not found: $SERVICE_DIR (set SERVICE_DIR=...)" >&2
  exit 1
fi
reset_file
load_env "$SERVICE_DIR/.env"

echo
bold "  xAI (post generation)"
secret XAI_API_KEY "xAI API key - https://console.x.ai"
default XAI_BASE_URL "https://api.x.ai/v1"
default XAI_MODEL "grok-4-fast"

echo
bold "  Voyage (embeddings - required for cohort matching)"
secret VOYAGE_API_KEY "Voyage key - https://dash.voyageai.com. Without it, moments have no embeddings and cohort returns nothing."
default VOYAGE_EMBED_MODEL "voyage-3"
default EMBEDDING_DIM "1024"

echo
bold "  Postgres / Supabase (required - the memory store has no cohort)"
note "Use the Session pooler URI (IPv4). Dashboard -> Connect -> Session mode."
note "postgresql://postgres.PROJECT:PASSWORD@aws-0-REGION.pooler.supabase.com:5432/postgres"
secret DB_CONNECTION_STRING "Supabase session-pooler connection string"
default SUPABASE_REGION "ca-central-1"
default SUPABASE_PROJECT_REF ""

echo
bold "  X / Twitter (live posts)"
secret X_BEARER_TOKEN "X API bearer token"
secret X_CONSUMER_KEY "X consumer key" optional
# config.py:71 reads X_CONSUMER_KEY_SECRET only. Older .env files use
# X_CONSUMER_SECRET, which therefore never loads. Mirror before prompting.
MIRRORED=0
if have X_CONSUMER_SECRET && ! have X_CONSUMER_KEY_SECRET; then
  put X_CONSUMER_KEY_SECRET "$(getv X_CONSUMER_SECRET)"; MIRRORED=1
fi
secret X_CONSUMER_KEY_SECRET "X consumer secret" optional
if [ "$MIRRORED" = 1 ]; then
  note "^ mirrored from X_CONSUMER_SECRET - config.py:71 only reads X_CONSUMER_KEY_SECRET"
fi
have X_CONSUMER_SECRET || put X_CONSUMER_SECRET "$(getv X_CONSUMER_KEY_SECRET)"
secret X_ACCESS_TOKEN "X access token" optional
secret X_ACCESS_TOKEN_SECRET "X access token secret" optional

echo
bold "  Bright Data (Google Trends - Workstream T)"
note "Zone serp_api1. Leave blank to keep trends disabled; everything else still works."
secret BRIGHTDATA_API_KEY "Bright Data API key - https://brightdata.com -> SERP API" optional
default BRIGHTDATA_ZONE "serp_api1"
default BRIGHTDATA_REQUEST_URL "https://api.brightdata.com/request"
put FEEDER_ENABLE_TRENDS "0"
note "FEEDER_ENABLE_TRENDS pinned to 0 until the T1 probe passes."

echo
bold "  Service flags"
default PORT "8787"
default FEEDER_ROLE "api"
default FEEDER_DEFAULT_USER "local"
default FEEDER_FIXTURE_MODE "0"
default FEEDER_ENABLE_X_SEARCH "1"
default FEEDER_ENABLE_SYNTH "1"
default FEEDER_ENABLE_IMAGINE "1"
default FEEDER_ENABLE_VOICE "1"
default FEEDER_API_KEY ""

echo
write_env "$SERVICE_DIR/.env"

# ══ CLIENT: grok-build/.env ═══════════════════════════════════════════════════
bold ""
bold "TUI client  ->  $CLIENT_DIR/.env"
reset_file
load_env "$CLIENT_DIR/.env"

device_name() {
  local n=""
  command -v scutil >/dev/null 2>&1 && n="$(scutil --get LocalHostName 2>/dev/null || true)"
  [ -z "$n" ] && n="$(hostname -s 2>/dev/null || hostname 2>/dev/null || echo device)"
  printf '%s' "$n" | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-+/-/g'
}
DEV="$(device_name)"; [ -z "$DEV" ] && DEV="device"

echo
bold "  Feeder API"
plain FEEDER_BASE_URL "Feeder API base URL" "$API_DEFAULT"
default FEEDER_URL "$(getv FEEDER_BASE_URL)"
default GROK_FEEDER "1"

echo
bold "  Identity (Workstream C reads these; ~/.feeder/identity.json takes over later)"
plain FEEDER_HANDLE "your display handle" "${USER:-$DEV}"
default FEEDER_USER_ID "$(getv FEEDER_HANDLE)-$DEV"
ok "FEEDER_USER_ID=$(getv FEEDER_USER_ID)"
note "Demo personas per shell: FEEDER_USER_ID=alice FEEDER_HANDLE=alice ./scripts/start-feeder.sh"
default FEEDER_API_KEY ""

echo
write_env "$CLIENT_DIR/.env"

# ══ VERIFY ════════════════════════════════════════════════════════════════════
bold ""
bold "Live API check"
BASE="$(getv FEEDER_BASE_URL)"
if command -v curl >/dev/null 2>&1; then
  H="$(curl -fsS -m 15 "$BASE/health" 2>/dev/null || true)"
  if [ -n "$H" ]; then
    ok "$BASE/health"
    printf '    %s\n' "$H"
    case "$H" in
      *'"fixture_mode":true'*) note "fixture_mode=true - live sources are OFF on that deploy." ;;
    esac
  else
    miss "$BASE/health unreachable"
  fi
else
  note "curl not found; skipped"
fi

bold ""
if [ -n "$MISSING" ]; then
  bold "Missing required keys - fill these before running the service locally:"
  for k in $MISSING; do miss "$k"; done
  echo
  note "Re-run any time; existing values are preserved."
  note "Without DB_CONNECTION_STRING + VOYAGE_API_KEY the local service falls back to"
  note "the in-memory store, and cohort matching cannot work."
  exit 1
fi
bold "All required keys present."
note "Never commit .env - both files are mode 600; .bak copies were left beside them."
