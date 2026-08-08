# george-plan.md — Sidecar, George's side of the plan

Abi is writing a user spec / feature model separately; this is my half.
Convergence: we reconcile the two docs at the table, disagreements resolved
in person, results land as amendments to both. Nothing here overrides his
spec — where they conflict, we talk.

## What we're building (as I understand it today)

A live feed inside the forked grok-build TUI, built from the user's
current working context — real X posts, LLM digests, and generated media
only when justified. Wakes while the agent works, freezes on typing.
Full detail: consolidated design in the grokathon repo
(`docs/SIDECAR.md` @ feature/sidecar) — key decisions repeated here so
this doc stands alone.

## Locked (from the earlier consolidation — relitigate only in person)

- Fork the TUI (`ActiveView::Sidecar` cloned from AgentDashboard pattern);
  plugin system has no UI surface.
- Live ambient feed (wake on agent-grind, freeze on keystroke), with
  guardrails: `AI-generated` tags always, query-scoped content, curator
  can HOLD, no engagement-bait replenishment.
- Curator actions: pull / digest / render(+justification) / hold.
- Build-in-public: draft from real diff only, diff-adjacent confirm,
  never auto-post. Link-free posts ($0.015 vs $0.200 with URL).
- ONE outbound secret filter on every payload leaving the machine.
- No code-gen cards tonight (v2).

## Implementation decisions (mine, new since consolidation)

1. **Brain is Python, deliberately boring.** Sync Flask + threads, no
   asyncio, no FastAPI/pydantic. Why: every piece is already verified in
   our starters TODAY (xAI chat/vision/Imagine/Voice clients AND X OAuth1
   posting via requests-oauthlib, tested 200). Deps capped at ~6, pinned.
2. **Two-speed sourcing** (from docs research, see `docs/PRIMITIVES.md`):
   `GET /2/tweets/search/recent` (seconds, $0.005) is the prefetch
   workhorse; xAI `x_search` (2-4 min) demotes to background semantic
   enrichment; filtered stream (6-7s push, rules rewritten as context
   shifts) is the live-mode option.
3. **Ranking:** port the scoring SHAPE of xai-org/x-algorithm (Apache-2.0):
   weighted multi-action combiner → author-diversity decay → OON
   multiplier → age cutoff, with public_metrics rates as proxies. Honest
   claim only: "X's open-source scoring architecture with public
   engagement proxies" — the real weights are unpublished.

## Anti-collision structure

- **Abi's zone:** the Rust tree. Mine: `sidecar/` + these docs + `build/`.
  Shared + frozen: `contract/` (change = both ack in person + VERSION bump,
  v1 fixtures never edited in place).
- **My agent lanes** (each Claude session owns its dirs, enforced via
  CLAUDE.md):
  - Lane A: `brain/server/` + `brain/distill/`
  - Lane B: `brain/xsearch/` + `brain/feed/`
  - Lane C: `brain/gen/` + `brain/post/`
  - Integrator (me): `brain/__main__.py`, `brain/core.py`, docs, merges.
- **Task boards sharded by writer:** `build/LANE-{A,B,C}.md` (one writing
  session each), `BUILD.md` index + `build/INTEGRATION.md` (integrator).
  Units: ≤60 min, files within lane dirs, acceptance command whose exit
  code defines done, demo-visible flag. Lanes run as /loop sessions:
  pull → topmost eligible unit → implement → acceptance green → check box
  → commit `[lane-x] ID: note` → push lane branch. Integrator merges
  lanes → sidecar-main every ~45 min.
- Branches: `sidecar-main` = integration (Abi commits TUI work here or
  tui/* — his call). My lanes: `brain/a|b|c` in worktrees.

## Dogfood + testing (designed by agent, adopted)

- **M0 (in this push, working):** brain watches THIS repo's git state,
  serves "you edited X 2m ago" cards. Real feed, zero AI. Later layers
  stack; M0 never regresses — feed never goes blank.
- `make peek` = watch-mode terminal viewer (demo surface until the TUI
  pane lands). `make fixtureserve` = fixtures on :7717 so Abi never
  blocks on me.
- Checkpoint ritual: `make test` + `make contract` + `make smoke` green
  + 10s manual peek glance → tag `demo-<HHMM>-<brain|tui|both>-<note>`.
  Hard tags 4pm/6pm/8pm/9:15pm. Demo runs off newest `both-*` tag, never
  HEAD.
- Two machines: Abi's TUI → my brain over LAN (`SIDECAR_URL`), fallback
  hotspot, fallback fixtureserve.

## Demo + hosting

- Stage demo: local, projected, fixture repo, scripted beats (wake →
  digest → freeze-on-typing → justified render → draft approve).
- **Most Users play:** static web page rendering `/cards` + analytics
  counter, tunneled via `cloudflared` from my laptop; demo-driver script
  keeps a fixture repo visibly active so the public feed always moves.
- **ttyd flex (stretch, go/no-go 7:30pm):** read-only browser terminal
  mirroring the real TUI via fixed-size tmux; xterm.js can't render
  inline images so the renderer's text-card fallback shows (needed for
  compat anyway); never part of the rehearsed script.
- Upstream story for the pitch: xai-org accepts no PRs — fork surface is
  thin + feature-flagged (`SIDECAR=1`), and we hand xAI the one-hook diff
  that would make this a plugin instead of a fork.

## Cut ladder (in order) & kill-risks

Cut: ttyd → Imagine render cards → click-to-import → voice briefing →
build-in-public. Never cut: distiller, real cards via prefetch,
wake/freeze, AI-generated tags, secret filter.

Risks: TUI fork sprawl (fallback: peek pane renders the demo) ·
x_search/recent-search relevance (curation iteration by 4:30) · quota
pool (caps + console checks 4pm/7pm) · **Access Token still Read-only —
regen to R+W before 6:30pm or build-in-public demos mocked**.

## Open for convergence with Abi's spec

- Feature list + priorities (his doc wins on user-facing scope)
- Card kinds beyond the current five, if his spec adds any
- Hotkey/UX conventions, wake/freeze thresholds
- Who integrates at 5:30 and demo roles
