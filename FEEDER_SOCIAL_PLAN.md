# Feeder → "Social Media for Work" — Implementation Plan

**Status:** ready to pick up. **Jam multiplayer is explicitly out of scope** (see §0.3).
**Audience:** an implementing agent fanning work out to subagents.

---

## 0. Ground truth (verified 2026-08-08, do not re-derive)

### 0.1 Repos

| Role | Path | Branch | Notes |
|---|---|---|---|
| Service | `/Users/george/Dev/grokathon` | `feature/feeder` | **Was left on this branch** by exploration; original was `feature/jam-api` |
| Client (TUI) | `/Users/george/Dev/grok-build` | `feature/social-media` | Byte-identical to `feature/feeder` (0 commits diverged) |

Other worktrees (`grok-build-abi-feeder`, `grok-build-demo`, `grokathon-feeder`) are stale checkouts. Ignore them.

### 0.2 What is live

| Service | URL | State |
|---|---|---|
| **feeder-api** | `https://feeder-api-production.up.railway.app` | `build=feeder-social-v2`. Feed + `/v1/sessions/*`. **Build against this one.** |
| jam-api | `https://jam-api-production.up.railway.app` | `jams=true`, also has sessions. **Do not touch.** Leave running as rollback. |

### 0.3 Out of scope

Jam multiplayer (`app/jam/`, `app/api/jams.py` on `grokathon@feature/jam-api`, ~2,500 lines) is **shelved, not deleted**. Do not merge it, do not deploy over `jam-api`. The `george/jam-tui-multiplayer` TUI branch stays parked.

Cohort replaces jam as the "other people exist" surface — asynchronous, no pairing required.

### 0.4 What Abi already built (all live, verified)

- **Schema** (`migrations_0001_init.sql`): `feeder_users`, `feeder_sessions`, `moments` (HNSW embedding index) on top of `feed_items` / `impressions` / `feedback` / `query_fingerprints`.
- **Routes** (`app/api/routes.py`): `POST /v1/sessions`, `/{id}/heartbeat`, `/{id}/moment`, plus feed query/post/ingest/feedback.
- **Store** (`app/db.py`): `upsert_user`, `upsert_moment`, `find_similar_moments`, `items_engaged_by_users`, `add_mute_tokens`, `add_muted_author`, `get_user_prefs`.
- **Mixer** (`app/mixer.py`): `build_query_pack` → `fanout_x_search` → `hotness` → `merge_and_score` (`sim*0.6 + hot*0.3 + penalty`, `×boost`).
- **Cohort is already wired into the query path** at `routes.py:218-232`.

### 0.5 The three defects this plan fixes

1. **`app/feed_slice.py:31`** — `_LAST_CONTEXT` is a single process-global. Written at `routes.py:181` via `remember_context(ctx)` with no user attached. Whoever queried last owns the server's idea of "current work"; the background generator writes for that person only. **Fatal to a multi-user product.**
2. **Identity** — the TUI defaults to the literal string `"local"`; server falls back to `FEEDER_DEFAULT_USER=local`. Everyone is the same user.
3. **Cohort is invisible** — `routes.py:226-231` merges peer-engaged items *flat* into `candidates`. `sources_used` gets `"cohort"` in aggregate, but no per-item marker survives, so the client cannot label them. All of the value, none of the visibility.

### 0.6 Env

Run `./scripts/setup-feeder-env.sh` first. Known gaps on this machine:
- `VOYAGE_API_KEY` — **missing**. No embeddings ⇒ `find_similar_moments` cannot match ⇒ cohort returns nothing.
- `DB_CONNECTION_STRING` — **missing**. Local service silently falls back to the in-memory store, which has no cohort.
- `BRIGHTDATA_API_KEY` — **missing**. Gates Workstream T only.
- `X_CONSUMER_SECRET` is set but `config.py:71` only reads `X_CONSUMER_KEY_SECRET`. The setup script mirrors it.

**Consequence:** until DB + Voyage creds exist, Workstreams S and C must be verified against `feeder-api-production`, not localhost.

---

## 1. Product thesis (what every task serves)

> The feed knows **what** you're building (moments), **who else** is building it (cohort), and **whether it's heating up** (trends).

The demo line, which no single workstream produces alone:

> **"4 people near you hit this today · rising 40% this week"**

---

## 2. Workstreams

Four lanes, partitioned **by file ownership** so subagents never edit the same file. Ownership is strict: if a task needs a file another lane owns, it is deferred to the integration step (§4).

```
        ┌─────────────────────────────────────────┐
  S ────┤ grokathon: feed_slice.py, workers/, db.py
        └─────────────────────────────────────────┘
        ┌─────────────────────────────────────────┐
  A ────┤ grokathon: routes.py, models.py, mixer.py
        └─────────────────────────────────────────┘
        ┌─────────────────────────────────────────┐
  C ────┤ grok-build: crates/**/views/feeder/*, scripts/
        └─────────────────────────────────────────┘
        ┌─────────────────────────────────────────┐
  T ────┤ grokathon: NEW files only (sources/, trends)
        └─────────────────────────────────────────┘
```

S, A, C, T all start in parallel. **T is gated on `BRIGHTDATA_API_KEY`** but its probe (T1) needs nothing else.

---

### Workstream S — Per-user context isolation
**Owns:** `app/feed_slice.py`, `app/workers/*`, `app/db.py`
**Effort: `ultracode`.** Touches background threads and a shared global; the failure mode is a silent cross-user data leak, not a crash.

| # | Task |
|---|---|
| S1 | Replace `_LAST_CONTEXT` (`feed_slice.py:31`) with a per-user map. Bound it (LRU or TTL) — this is a long-lived server, an unbounded dict keyed by user id is a leak. |
| S2 | `remember_context(context, *, user_id)` and `last_context(user_id)`. Keep a deprecated no-user shim **only** if needed to avoid breaking A's call site mid-flight; delete it at integration. |
| S3 | `generate_feed_slice(..., user_id=...)` — generated posts are attributed to the requesting user. |
| S4 | Background generation loop: iterate **recent rows in `moments`** instead of one global slot. Cap concurrency; do not spawn one thread per user. |
| S5 | Guard the map with a lock. `routes.py:238-250` spawns generation on a `threading.Thread` per query — concurrent mutation is real, not theoretical. |

**Acceptance**
- Two concurrent queries (alice/OAuth, bob/Rust) produce generated posts on their own topic. Neither sees the other's.
- Restarting the service does not resurrect a stale global context.
- 50 sequential distinct user ids do not grow the map without bound.

**Verify**
```bash
# two users, interleaved, then inspect generated attribution
curl -sX POST $BASE/v1/feed/query -H 'content-type: application/json' \
  -H 'X-User-Id: alice' -d '{"user_id":"alice","context":{"recent_prompts":["oauth pkce refresh token"]},"limit":5}'
curl -sX POST $BASE/v1/feed/query -H 'content-type: application/json' \
  -H 'X-User-Id: bob'   -d '{"user_id":"bob","context":{"recent_prompts":["rust tokio select loop"]},"limit":5}'
```

---

### Workstream A — Identity + cohort attribution (server)
**Owns:** `app/api/routes.py`, `app/models.py`, `app/mixer.py`
**Effort: standard.** Mechanical, but A3 is the single highest-value change in the plan.

| # | Task |
|---|---|
| A1 | Call `upsert_user` on **every** `/v1/feed/query`, not only session creation. Currently a user only exists if they created a session. |
| A2 | Accept optional `handle` on `SessionRequest` and `QueryRequest`; persist to `feeder_users.handle`. Author bylines render the handle, never a raw uuid. |
| A3 | **Tag cohort items.** At `routes.py:226-231`, before `merge_candidates(candidates, engaged)`, stamp each engaged item with a per-item marker — reuse the existing `reason_chips` convention from `mixer.py` — carrying the peer count (`len(cohort_user_ids)`). This is what makes cohort visible. |
| A4 | Surface cohort in the query response: per-item `source: "cohort"` and `peer_count`, so C can render without guessing. |
| A5 | `/health` reports `sessions: true`, `cohort: true`, `trends: <flag>`. Keep Abi's `build` stamp — it is how you tell which image is live. |

**Acceptance**
- A user who never created a session still appears in `feeder_users` after one query.
- A query response contains at least one item with `source: "cohort"` and a peer count, given two users with overlapping moments.
- Post bylines show `@handle`.

**Coordination:** A owns the single call site `remember_context(ctx)` at `routes.py:181`. When S lands S2, A updates that line. Nobody else edits `routes.py`.

---

### Workstream C — Client identity + cohort rendering
**Owns:** `crates/**/xai-grok-pager/src/views/feeder/*`, `src/app/dispatch/feeder.rs`, `src/slash/commands/feeder.rs`, `scripts/start-feeder.sh`
**Effort: standard** for C1–C4, **`ultracode` for C5** (rendering inside a live TUI dock with existing layout constraints).

| # | Task |
|---|---|
| C1 | New `views/feeder/identity.rs`. Resolution order: `FEEDER_USER_ID` env → `~/.feeder/identity.json` → mint. Mint = `{device}-{uuid8}` where device derives from `scutil --get LocalHostName` (the `device_name()` logic in `scripts/start-feeder.sh:22-36` already does this — port it, don't reinvent). Persist handle + device_id. |
| C2 | `feed_client.rs::from_env` uses `identity`. **Never fall back to bare `"local"` when a hostname is available.** Expose `handle` and send it on session create and query. |
| C3 | `scripts/start-feeder.sh`: print a startup banner — who you are, API base, health flags, identity file path. |
| C4 | Confirm session paths are `/v1/sessions/{id}/heartbeat` and `/v1/sessions/{id}/moment`. Commit `daf34e6` claims this is fixed; **verify against the live API before assuming**. Keep the 45s query timeout from `8a919c2` — multi-X fanout genuinely needs it. |
| C5 | Render the cohort chip on items where `source == "cohort"`: *"4 people near you hit this today."* Must fit the existing dock width; do not let it push media out of the panel. |

**Acceptance**
- Two terminals, `FEEDER_USER_ID=alice` / `=bob`, are distinct users server-side.
- Deleting `~/.feeder/identity.json` mints a new id; restarting without deleting keeps the old one.
- Bare `cargo run` with no env produces a hostname-derived id, never `"local"`.
- Cohort chips appear and the dock layout is unchanged otherwise.

---

### Workstream T — Google Trends (Bright Data)
**Owns:** `app/sources/brightdata.py` *(new)*, `app/trends.py` *(new)*, `migrations_0004_trends.sql` *(new)*, `scripts/probe_brightdata_trends.py` *(new)*
**Gated on `BRIGHTDATA_API_KEY`.** All-new files ⇒ zero merge conflict with S/A/C. The only shared-file change (T5) is deferred to integration.
**Effort: `ultracode` for T4–T5** (ranking interacts with `merge_and_score`'s existing weights; getting α wrong silently wrecks relevance).

| # | Task |
|---|---|
| T1 | **Probe first.** `scripts/probe_brightdata_trends.py` against the live key. Zone `serp_api1`, URL must carry `brd_json=1&brd_trends=timeseries,geo_map`. Record: payload shape, latency, error modes. **Everything below depends on this answer — do not write T2+ before T1 returns real data.** |
| T2 | `Settings` fields in `app/config.py`: `brightdata_api_key`, `brightdata_zone`, `brightdata_request_url`, `feeder_enable_trends`. ⚠️ `config.py` is unowned — coordinate; it is a 4-line addition. Flag off ⇒ zero network calls. |
| T3 | `trend_signals` cache table, ~12h TTL, refreshed **by a background worker only**. Never on the query path: `/v1/feed/query` already fans out to X and cannot afford another blocking call. |
| T4 | Term expansion — at most **one** related term added to `build_query_pack`. Respect existing mutes. |
| T5 | Rank prior: `score *= 1 + α·heat`, **α ≤ 0.1**. Slots in beside `hotness` in `merge_and_score`. *(mixer.py is owned by A — hand off at integration.)* |
| T6 | Trending rail in the dock, scoped to the user's own session terms. *(client files owned by C — hand off at integration.)* |

**Scope discipline — do not build:**
- Global "Trending Now" ingestion. Consumer Google trending is sports and celebrities; it poisons a dev feed.
- Synchronous SERP on the query path.
- A new `source_type`. Trends is a **signal on existing items**, plus one rail. It is not a card type.

**Acceptance**
- `FEEDER_ENABLE_TRENDS=0` ⇒ provably zero Bright Data network calls.
- Cache hit rate > 90% under normal query load.
- Rail shows terms traceable to the user's own session, never generic global trends.

---

## 3. Fan-out instructions

Spawn one subagent per lane, each with `isolation: "worktree"` so a failed lane cannot corrupt the others.

Every subagent prompt must carry:
1. Its **owned file list**, and an explicit instruction to edit nothing outside it.
2. §0 of this document verbatim (ground truth — stops re-derivation).
3. Its acceptance criteria.
4. `BASE=https://feeder-api-production.up.railway.app` for verification.
5. **"Do not push, deploy, or run migrations against production. Do not commit unless asked."**

Lanes S and A both live in `grokathon` and both ultimately touch `routes.py:181`. **A owns that line.** S must ship its new signature with a temporary back-compatible shim so the two lanes never block each other.

### ultracode invocations

`ultracode` is the dynamic-workflow trigger keyword and an effort level — **user-typed, not agent-triggerable.** Paste these yourself:

```
ultracode  Workstream S from FEEDER_SOCIAL_PLAN.md: replace the process-global
_LAST_CONTEXT in app/feed_slice.py with bounded per-user context, thread user_id
through generate_feed_slice, and rewrite the background loop to iterate recent
moments. Concurrency-safe — routes.py spawns a thread per query.
```

```
ultracode  Workstream T tasks T4-T5 from FEEDER_SOCIAL_PLAN.md: trend-heat rank
prior in merge_and_score, alpha <= 0.1, plus single-term query-pack expansion.
Show me the relevance impact before and after.
```

Everything else runs at standard effort.

---

## 4. Integration (serial, single agent, after all lanes report)

| # | Step |
|---|---|
| I1 | Merge lanes. Resolve `config.py` (T2) and `routes.py:181` (S2 shim → A's final signature). Delete the shim. |
| I2 | Hand off T5 into `mixer.py` and T6 into the client dock — the two cross-lane changes deliberately deferred. |
| I3 | Full-stack smoke, **local first**, two personas. |
| I4 | Update `.env.example` and the demo script. Kill the `~/.grok/config.toml` instructions in the demo doc — that file is fiction; it is start scripts and env vars now. |
| I5 | **Stop.** Deploy requires explicit human consent. |

### Deploy gate (human-approved only)

```bash
curl -s $BASE/health | jq '.build, .sessions, .cohort, .trends'
# must be the new build stamp, sessions/cohort true, trends false on first ship
```

Ship with `FEEDER_ENABLE_TRENDS=0`. Turn trends on only after T1+T3 have soaked.

---

## 5. Demo readiness

Cohort **returns nothing when only one person is using the system** — it is not broken, it has no peers to match. Before demo:

1. Two machines (or two shells with forced personas) against the same service.
2. Both generate moments on overlapping topics.
3. One engages an item; the other sees it chipped.

This is why Workstream C ships before any demo rehearsal.

---

## 6. Explicit non-goals

- Jam multiplayer merge or redeploy.
- Auth / RLS. Soft identity via `X-User-Id` is sufficient here.
- Trend cards, `geo_map` UI, global trending ingestion.
- Deploying over `jam-api-production`.
- Any push or deploy without explicit consent.
