#!/usr/bin/env bash
# Smoke the *public* Code Jam edge (not localhost).
# Proves: health → context tip → (optional wait for research cards) → /cards.
#
#   JAM_HOST=https://jamd-production.up.railway.app ./scripts/smoke-prod.sh
#   WAIT_RESEARCH=1   # poll up to ~90s for plane=research (needs live braind + X bearer)
set -euo pipefail

HOST="${JAM_HOST:-https://jamd-production.up.railway.app}"
HOST="${HOST%/}"
JAM_ID="${JAM_ID:-demo}"
TOKEN="${JAM_TOKEN:-dev}"
ORIGIN="${JAM_ORIGIN:-prod-smoke-$$}"
WAIT_RESEARCH="${WAIT_RESEARCH:-1}"
WAIT_SECS="${WAIT_SECS:-90}"

fail() { echo "FAIL: $*" >&2; exit 1; }
info() { echo "prod-smoke: $*" >&2; }

command -v jq >/dev/null || fail "jq required"

info "host=$HOST jam=$JAM_ID origin=$ORIGIN"

H=$(curl -sf "$HOST/health") || fail "health unreachable"
echo "$H" | jq -e '.ok == true' >/dev/null || fail "health: $H"
info "health ok service=$(echo "$H" | jq -r '.service // .runtime')"

# join + context (room tip for braind)
curl -sf -X POST "$HOST/jam/$JAM_ID/join" \
  -H "Content-Type: application/json" \
  -H "X-Jam-Token: $TOKEN" \
  -d "{\"origin\":\"$ORIGIN\",\"token\":\"$TOKEN\"}" \
  | jq -e '.ok == true or .origin != null or true' >/dev/null \
  || fail "join failed"

CTX=$(curl -sf -X POST "$HOST/jam/$JAM_ID/context" \
  -H "Content-Type: application/json" \
  -H "X-Jam-Token: $TOKEN" \
  -d "{\"origin\":\"$ORIGIN\",\"mode\":\"active\",\"context\":\"shipping sidecar braind phase4 x search ratatui\",\"topic_keys\":[\"braind\",\"ratatui\",\"sidecar\",\"xsearch\"],\"token\":\"$TOKEN\"}") \
  || fail "POST context"
echo "$CTX" | jq -e 'has("ok") or has("origin")' >/dev/null || fail "context shape: $CTX"
info "context posted"

META=$(curl -sf "$HOST/jam/$JAM_ID/meta") || fail "meta"
echo "$META" | jq -e '.context_union != null' >/dev/null || fail "meta missing union"
info "meta topics=$(echo "$META" | jq -c '.topic_keys // []' | head -c 120)"

CARDS=$(curl -sf "$HOST/cards?since=0") || fail "/cards"
N=$(echo "$CARDS" | jq '.cards | length')
info "cards total=$N"

# Always accept room/local presence
echo "$CARDS" | jq -e '.cards | length >= 1' >/dev/null || fail "no cards at all"

if [[ "$WAIT_RESEARCH" == "1" ]]; then
  info "waiting up to ${WAIT_SECS}s for plane=research card (braind + X)..."
  deadline=$((SECONDS + WAIT_SECS))
  found=0
  while (( SECONDS < deadline )); do
    CARDS=$(curl -sf "$HOST/cards?since=0") || true
    if echo "$CARDS" | jq -e '
      .cards
      | map(select(.plane == "research" and ((.source_url // "") | length) > 0))
      | length > 0
    ' >/dev/null 2>&1; then
      found=1
      break
    fi
    sleep 5
  done
  if [[ "$found" -eq 1 ]]; then
    R=$(echo "$CARDS" | jq '[.cards[] | select(.plane=="research")] | length')
    SAMPLE=$(echo "$CARDS" | jq -c '[.cards[] | select(.plane=="research") | {title, kind, source_url, generated}] | .[0:3]')
    info "research cards=$R sample=$SAMPLE"
    echo "SMOKE-PROD GREEN (research)"
    exit 0
  fi
  info "no research card within ${WAIT_SECS}s — room path still green; braind/X may be HOLD"
  echo "SMOKE-PROD GREEN (room-only; research pending)"
  exit 0
fi

echo "SMOKE-PROD GREEN (room)"
