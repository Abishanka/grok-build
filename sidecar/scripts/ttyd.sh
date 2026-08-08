#!/usr/bin/env bash
# Read-only browser mirror of the demo tmux session. GO/NO-GO at 7:30pm.
# brew install ttyd; tmux session "demo" must exist (fixed size 120x36).
exec ttyd --readonly -p 7681 tmux attach -rt demo
