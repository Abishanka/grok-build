#!/usr/bin/env bash
# Contract smoke: start brain, assert the surface, kill. Exit 0 = green.
set -euo pipefail
cd "$(dirname "$0")/.."
PY=${PY:-.venv/bin/python}
PORT=${SIDECAR_PORT:-7719}

SIDECAR_PORT=$PORT $PY -m brain &
BRAIN=$!
trap 'kill $BRAIN 2>/dev/null || true' EXIT

for i in $(seq 1 20); do
  curl -sf "localhost:$PORT/state" >/dev/null 2>&1 && break
  sleep 0.5
  [ "$i" = 20 ] && { echo "FAIL: /state never came up"; exit 1; }
done

curl -sf "localhost:$PORT/state" | jq -e '.mode and .context' >/dev/null || { echo "FAIL: state shape"; exit 1; }
CARDS=$(curl -sf "localhost:$PORT/cards?since=0")
echo "$CARDS" | jq -e '.cards | length >= 1' >/dev/null || { echo "FAIL: no cards"; exit 1; }
echo "$CARDS" | jq -e '.cards[] | select(.kind=="" or .title=="")' >/dev/null 2>&1 && { echo "FAIL: empty fields"; exit 1; }
MAX=$(echo "$CARDS" | jq '[.cards[].seq] | max')
curl -sf "localhost:$PORT/cards?since=$MAX" | jq -e '.cards | length == 0' >/dev/null || { echo "FAIL: since filter"; exit 1; }
curl -sf -X POST "localhost:$PORT/action" -H 'Content-Type: application/json' \
  -d '{"card_id":"c1","action":"dismiss"}' | jq -e '.ok' >/dev/null || { echo "FAIL: action"; exit 1; }
BRIEF=$(curl -s -o /dev/null -w "%{http_code}" "localhost:$PORT/briefing.wav")
[ "$BRIEF" = "200" ] || [ "$BRIEF" = "503" ] || { echo "FAIL: briefing $BRIEF"; exit 1; }

echo "SMOKE GREEN"
