# BUILD — index (integrator-owned; lanes: READ ONLY)

Boards are sharded by writer. A lane session edits ONLY its own board +
its own dirs. Integrator merges lane branches → sidecar-main ~every 45min.

- `build/LANE-A.md` — server + distill (context, wake/freeze, M-milestones)
- `build/LANE-B.md` — xsearch + feed (sourcing, ranking, curator)
- `build/LANE-C.md` — gen + post (filter, posting, hosting, voice, renders)
- `build/INTEGRATION.md` — merge log + checkpoint tags

Unit protocol (each /loop iteration): pull lane branch → topmost unchecked
unit with satisfied deps → implement inside owned dirs → run `accept:`
until exit 0 → flip checkbox + 1-line note → commit `[lane-x] ID: note` →
push lane branch.

Checkpoint (integrator): test+contract+smoke green + 10s `make peek`
glance → tag `demo-<HHMM>-<brain|tui|both>-<note>`. Hard tags:
4pm / 6pm / 8pm / 9:15pm. Demo runs from newest `both-*` tag, never HEAD.

Cut ladder: ttyd → render cards → click-to-import → voice → build-in-
public. Never cut: distiller, real cards, wake/freeze, AI-gen tags,
secret filter.
