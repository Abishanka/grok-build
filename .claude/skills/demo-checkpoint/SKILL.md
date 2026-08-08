---
name: demo-checkpoint
description: Commit the current working state as a demo-safe checkpoint (tagged, with a note on what's demoable). Manually triggered with /demo-checkpoint — not auto-invoked.
disable-model-invocation: true
---

Run this whenever the project reaches a new working milestone, so there's
always a known-good fallback to revert to if a later change breaks the demo.

Steps:
1. Run `git status` and `git diff` to see what's changed. If nothing's
   changed, say so and stop.
2. Sanity-check the app still runs (use whatever the run command is for the
   current stack — check `CLAUDE.md` / `Makefile`) before committing. Do not
   checkpoint a broken state.
3. `git add -A` (excluding anything `.gitignore` already excludes — never
   force-add `.env`).
4. Commit with a message describing what's now demoable, e.g.
   `checkpoint: live Grok chat + tool call working end to end`.
5. Tag it: `git tag demo-checkpoint-$(date +%H%M)`.
6. Report back in one line what's now safely checkpointed.
