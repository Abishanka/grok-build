#!/usr/bin/env bash
# Inject X API credentials into .env, names matched to the X developer
# portal's "Keys & Tokens" page. Run it YOURSELF in a terminal (prompts
# are hidden); paste each value, Enter to skip any you don't have.
set -euo pipefail
ENV_FILE="$(cd "$(dirname "$0")/.." && pwd)/.env"

put() {
  local name="$1" value="$2"
  [ -z "$value" ] && return 0
  grep -v "^${name}=" "$ENV_FILE" 2>/dev/null > "$ENV_FILE.tmp" || true
  printf '%s=%s\n' "$name" "$value" >> "$ENV_FILE.tmp"
  mv "$ENV_FILE.tmp" "$ENV_FILE"
  echo "  set $name"
}

echo "Paste from developer portal > Keys & Tokens (input hidden, Enter to skip):"
read -rs -p "Bearer Token (App-Only Authentication): " v; echo; put X_BEARER_TOKEN "$v"
read -rs -p "Consumer Key (OAuth 1.0): " v; echo; put X_CONSUMER_KEY "$v"
read -rs -p "Consumer Secret (OAuth 1.0): " v; echo; put X_CONSUMER_SECRET "$v"
read -rs -p "Access Token (OAuth 1.0): " v; echo; put X_ACCESS_TOKEN "$v"
read -rs -p "Access Token Secret (OAuth 1.0): " v; echo; put X_ACCESS_TOKEN_SECRET "$v"

chmod 600 "$ENV_FILE"

set -a; source "$ENV_FILE"; set +a
if [ -n "${X_BEARER_TOKEN:-}" ]; then
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $X_BEARER_TOKEN" \
    "https://api.x.com/2/users/by/username/xai")
  echo "Bearer token check: HTTP $code (200 = working, 401 = bad paste)"
fi
echo "Done. New shells pick this up automatically; current shell: source scripts/env.sh"
