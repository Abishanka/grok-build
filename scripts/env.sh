#!/usr/bin/env bash
# source this to load .env into your shell: `source scripts/env.sh`
# Works under bash and zsh: bash has BASH_SOURCE; zsh sets $0 to the
# sourced file's path by default.
_env_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
set -a
# shellcheck disable=SC1091
[ -f "$_env_dir/../.env" ] && source "$_env_dir/../.env"
set +a
unset _env_dir

if [ -z "$XAI_API_KEY" ]; then
  echo "XAI_API_KEY is not set — copy .env.example to .env and fill it in." >&2
fi
