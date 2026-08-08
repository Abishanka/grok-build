#!/usr/bin/env bash
# PreToolUse hook: blocks Edit/Write to .env* and .git internals.
# Exit 2 = block the tool call (Claude Code convention).
input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

if [[ "$file_path" == *".env"* ]] || [[ "$file_path" == *"/.git/"* ]]; then
  echo "Blocked: refusing to write to $file_path (secrets/VCS guard)" >&2
  exit 2
fi
exit 0
