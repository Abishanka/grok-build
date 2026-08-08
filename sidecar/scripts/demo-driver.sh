#!/usr/bin/env bash
# Keeps a fixture repo visibly active so the feed always shows movement.
# Usage: ./demo-driver.sh /path/to/fixture-repo
set -euo pipefail
REPO=${1:?fixture repo path}
i=0
while true; do
  i=$((i+1))
  echo "// demo edit $i $(date +%H:%M:%S)" >> "$REPO/activity.log"
  git -C "$REPO" add -A && git -C "$REPO" commit -qm "demo activity $i" || true
  sleep 45
done
