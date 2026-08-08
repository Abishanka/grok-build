# Sidecar (grok-build fork) — hackathon build, judging ~10pm tonight

Upstream Rust tree = Abi's zone. `sidecar/` (Python brain) + `docs/` +
`build/` = George's zone. `contract/` is FROZEN — changes need both
humans + VERSION bump; v1 fixtures are never edited in place.

Plans: `docs/george-plan.md` (George) — Abi's spec lands separately;
convergence happens in person. API ground truth: `docs/PRIMITIVES.md`.

## Lane ownership (agent sessions: only edit YOUR dirs)
- Lane A: `sidecar/brain/server/`, `sidecar/brain/distill/` (+ board `build/LANE-A.md`)
- Lane B: `sidecar/brain/xsearch/`, `sidecar/brain/feed/` (+ `build/LANE-B.md`)
- Lane C: `sidecar/brain/gen/`, `sidecar/brain/post/` (+ `build/LANE-C.md`)
- Integrator ONLY: `brain/__main__.py`, `brain/core.py`, `contract/`, docs, `BUILD.md`
Loop: pull → topmost eligible unit in YOUR board → implement → run its
`accept:` command until exit 0 → check box → commit `[lane-x] ID: note`.

## Boring-tech rules (prevent the spiral)
- Python SYNC only: Flask + threads + locks. NO asyncio, ever.
- No FastAPI/pydantic/ORM. Deps stay in `sidecar/requirements.txt`, ~6 max.
- Three similar lines beat a premature helper. Ship the demo path.

## Commands
- `cd sidecar && make run` — brain on :7717 (SIDECAR_REPO defaults to this repo)
- `make peek` / `make test` / `make contract` / `make smoke` / `make fixtureserve`
- Checkpoint only when test+contract+smoke green AND a 10s peek glance
  shows a card describing what you just did → tag `demo-<HHMM>-<scope>-<note>`.

## Secrets
- Never print/read `.env` into chat; hooks block it. Key-shaped strings
  never go in commands (pbpaste pattern instead).
- Every payload leaving the machine passes `brain/post/filter` once built.

## Shared plan
- **SoT for tonight:** `docs/CODE-JAM.md` (phases, `/sidecar`, checkpoints).
- `docs/george-plan.md` / `PRIMITIVES.md` still useful for brain ops and APIs.

## Shared plan (SoT)

- **Single source of truth:** [`docs/SIDECAR.md`](docs/SIDECAR.md) — phases 1–4, `/sidecar` entry, checkpoints, contract, split.
- API detail: `docs/PRIMITIVES.md` · wire: `contract/`
- Historical: `docs/CODE-JAM.md`, `docs/george-plan.md` (superseded)
