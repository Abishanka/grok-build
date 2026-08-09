#!/usr/bin/env bash
# Launch the Feeder social stack in fresh terminal windows (Ghostty-aware).
#
#   ./scripts/dev-social-stack.sh --service            # API window only
#   ./scripts/dev-social-stack.sh --user alice         # TUI window as "alice"
#   ./scripts/dev-social-stack.sh --user bob           # second persona
#   ./scripts/dev-social-stack.sh --demo               # service + alice + bob
#   ./scripts/dev-social-stack.sh --user carol --here  # run in THIS window
#
# Two windows with different --user are two distinct users to the server:
# cohort keys off user_id, which is just an env var. No second laptop needed.
#
# Env layering (later wins):
#   ~/.feeder/social-dev.env      DB + Voyage (pulled from Railway)
#   ../grokathon/.env             YOUR X / xAI keys — these override
#
# Env:
#   PORT   service port (default 8788)

set -euo pipefail

CLIENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_DIR="${SERVICE_DIR:-/Users/george/Dev/grokathon-social}"
VENV_PY="${VENV_PY:-/Users/george/Dev/grokathon/.venv/bin/python}"
BASE_ENV="${BASE_ENV:-$HOME/.feeder/social-dev.env}"
USER_ENV="${USER_ENV:-/Users/george/Dev/grokathon/.env}"   # your own X / xAI keys
LAUNCH_LIST=""
SCRATCH="${TMPDIR:-/tmp}/feeder-launch"
PORT="${PORT:-8788}"
BASE="http://127.0.0.1:${PORT}"

DO_SERVICE=0; DO_TUI=0; HERE=0; PERSONA=""
[ $# -eq 0 ] && { DO_SERVICE=1; DO_TUI=1; }
while [ $# -gt 0 ]; do
  case "$1" in
    --service) DO_SERVICE=1 ;;
    --user)    DO_TUI=1; PERSONA="${2:-}"; shift ;;
    --tui)     DO_TUI=1 ;;
    --demo)    DO_SERVICE=1; DO_TUI=2 ;;      # 2 = alice + bob
    --here)    HERE=1 ;;
    -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
  shift
done

die() { printf '\033[31m%s\033[0m\n' "$1" >&2; exit 1; }
ok()  { printf '  \033[32m/\033[0m %s\n' "$1"; }
note(){ printf '  \033[2m%s\033[0m\n' "$1"; }

[ -d "$SERVICE_DIR" ] || die "service worktree missing: $SERVICE_DIR"
[ -x "$VENV_PY" ]     || die "python venv missing: $VENV_PY"
[ -f "$BASE_ENV" ]    || die "env file missing: $BASE_ENV"
mkdir -p "$SCRATCH"

# ── open a command in a new terminal window ──────────────────────────────────
# Prepares a launch script per persona.
# Writes a runnable script and tells you to paste it. It does NOT drive your
# terminal: `open -n` spawns a second Ghostty instance, and Ghostty restores
# saved window state per instance, so each call multiplied the windows.
new_window() { # new_window <slug> <title> <shell-command>
  local slug="$1" title="$2" cmd="$3"
  local f="$SCRATCH/$slug.sh"
  {
    echo '#!/bin/bash'
    echo "printf '\\033]0;${title}\\007'"
    echo "$cmd"
  } > "$f"
  chmod +x "$f"
  LAUNCH_LIST="${LAUNCH_LIST}
  \033[1m${title}\033[0m
    ${f}"
}

# USER_ENV first, BASE_ENV second: BASE_ENV holds verified-working keys.
# (grokathon/.env's X_BEARER_TOKEN returns 401 — truncated, 110 vs 114 chars.)
# To use your own key, fix it there and swap these two.
ENV_PRELUDE="set -a; [ -f '$USER_ENV' ] && . '$USER_ENV'; . '$BASE_ENV'; set +a"

printf '\033[1m\nFeeder social stack\033[0m\n'
note "service : $SERVICE_DIR ($(git -C "$SERVICE_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?'))"
note "tui     : $CLIENT_DIR ($(git -C "$CLIENT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?'))"
note "api     : $BASE"
if [ -f "$USER_ENV" ]; then
  note "keys    : $USER_ENV then $BASE_ENV (verified keys win)"
else
  note "keys    : $BASE_ENV only ($USER_ENV not found)"
fi
echo

# ── service ──────────────────────────────────────────────────────────────────
SERVICE_LOG="$SCRATCH/service.log"
if [ "$DO_SERVICE" = 1 ]; then
  if curl -sf --max-time 3 "$BASE/health" >/dev/null 2>&1; then
    ok "service already up on $PORT"
  else
    # Detached, logging to a file — no window needed.
    nohup bash -c "cd '$SERVICE_DIR'; $ENV_PRELUDE; export PORT=$PORT; exec '$VENV_PY' -m uvicorn app.main:app --host 127.0.0.1 --port $PORT --log-level info" \
      > "$SERVICE_LOG" 2>&1 &
    echo $! > "$SCRATCH/service.pid"
    printf '  starting service '
    for _ in $(seq 1 45); do
      curl -sf --max-time 2 "$BASE/health" >/dev/null 2>&1 && break
      printf '.'; sleep 1
    done; echo
    if ! curl -sf --max-time 3 "$BASE/health" >/dev/null 2>&1; then
      printf '\033[31m  service failed to start. Last lines:\033[0m\n'
      tail -20 "$SERVICE_LOG" 2>/dev/null | sed 's/^/    /'
      exit 1
    fi
    ok "service up (pid $!)"
    note "$(curl -s --max-time 3 "$BASE/health")"
  fi
  note "logs: tail -f $SERVICE_LOG    (watch for 'cohort moments=N users=N engaged=N')"
  note "stop: kill \$(cat $SCRATCH/service.pid)   # only the pid we started"
fi

# ── TUI ──────────────────────────────────────────────────────────────────────
launch_tui() { # launch_tui <handle>
  local h="$1"
  local cmd="cd '$CLIENT_DIR'
$ENV_PRELUDE
export FEEDER_BASE_URL='$BASE' FEEDER_URL='$BASE'
export FEEDER_USER_ID='$h' FEEDER_HANDLE='$h' GROK_FEEDER=1
echo \"feeder: @$h -> $BASE\"
exec cargo run -p xai-grok-pager-bin"
  if [ "$HERE" = 1 ]; then
    eval "$cmd"
  else
    new_window "tui-$h" "grok-build @$h" "$cmd"
    ok "launcher ready for @$h"
  fi
}

if [ "$DO_TUI" = 2 ]; then
  launch_tui alice
  launch_tui bob
elif [ "$DO_TUI" = 1 ]; then
  launch_tui "${PERSONA:-${USER:-dev}}"
fi

if [ -n "${LAUNCH_LIST:-}" ]; then
  printf '\n\033[1mOpen a Ghostty tab (Cmd+T) for each and run:\033[0m\n'
  printf '%b\n' "$LAUNCH_LIST"
fi

echo
note "in the TUI:  /feeder  ·  j/k move  ·  s save  ·  u use  ·  r refresh  ·  q close"
note "first cargo build is slow; later tabs reuse the cache"
