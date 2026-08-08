# SIDECAR — consolidated build plan (single SoT)

**Status:** single source of truth on `sidecar-main`  
**Repo:** https://github.com/Abishanka/grok-build  
**Owners:** **Abi** = Rust pager/TUI · **George** = `sidecar/` brain + jam API · **Both** = contract, gates, demo  
**Supporting:** `docs/PRIMITIVES.md` · `contract/*`  
**Creds:** shell/zshrc only · never commit · never print  
**Both:** `git pull` this file before coding; tag every checkpoint  

---

## 0. One-liner

**Sidecar** is a feed inside **grok-build**, opened with **`/sidecar`**, about *what you’re building* — real X posts, digests, room activity — that **wakes while the agent works** and **freezes when you type**.  

**Code Jam** is the multiplayer evolution: shared session context steers one feed so the **room and audience perceive the coding** (Spaces-shaped, build-shaped). X is research input + optional approved outbound — not a mirrored firehose.

---

## 1. Locked decisions

| # | Decision |
|---|----------|
| 1 | **Fork pager** for UI: `ActiveView::Sidecar` + **`/sidecar` slash** (clone `/dashboard` pattern). Plugins **cannot** own panes. |
| 2 | **Entry = `/sidecar`** (not primary hotkey). Optional `/sidecar post`, `/sidecar leave`. Flag `SIDECAR=1` / `[sidecar].enabled`. |
| 3 | Plugin = optional hooks/skills/helpers only — never the feed surface. |
| 4 | **Brain = boring Python** (Flask + threads) on **`localhost:7717`**. |
| 5 | **Ambient attention:** wake on grind; **freeze on local keystroke**; AI-generated tags always; curator may HOLD. |
| 6 | **Build-in-public:** real diff only; dual-pane confirm; approve only; link-free X post; secret filter twice. |
| 7 | **No code-gen cards**, no For You endpoint, no auto-post feed to X. |
| 8 | **Anti-collision:** Abi = Rust tree · George = `sidecar/` + most docs · `contract/` frozen without both ack. |
| 9 | **Demo from checkpoint tags**, never dirty HEAD. |
| 10 | **Priority law (do not invert):** Phase 1 → 2 → 3 → 4. |

---

## 2. Priority law + phases

```
PHASE 1  TUI works (/sidecar + fixtures + freeze)
PHASE 2  Build-in-public → user X account
PHASE 3  Jam v0 = share context only
PHASE 4  Full Code Jam = union + X research feed + audience
         (+ optional A1 same-git / A2 sandbox)
```

No Phase N+1 until Phase N has a **green checkpoint tag** and a **30s show path**.

---

## 3. How `/sidecar` works in grok-build

| Mechanism | Native feed pane? |
|-----------|-------------------|
| Pager slash builtin (`/dashboard` style) | **Yes — use this** |
| Plugin commands/skills | No pane |
| Hooks | No pane (later: auto context push) |
| Hotkey-only | Avoid as primary |

**Precedent:**  
`src/slash/commands/dashboard.rs` → `Action::OpenDashboard` → `ActiveView::AgentDashboard`

**Implement:**  
`/sidecar` → `Action::OpenSidecar` → `ActiveView::Sidecar`  
Register in slash registry; feature-flag like dashboard; fullscreen-oriented.

**Fallback:** if TUI late → `peek`/web dual-pane labeled “Sidecar peek”, still hits `:7717`.

---

## 4. Checkpoint doctrine

Every checkpoint records:

1. ≤3 run commands  
2. 30s judge view  
3. Fake vs real  
4. Tag: `cp-P{n}-{HHMM}-{note}` on `sidecar-main`  
5. Who demos solo  

**On failure:** roll demo to last `cp-*-done` (or last green mini-CP). Never stage dirty HEAD.

---

## 5. Phase 1 — TUI works

### Build
| Who | Work |
|-----|------|
| **Abi** | `ActiveView::Sidecar`, `/sidecar` + leave, scroll, kind chrome, AI tags, FREEZE badge on keystroke |
| **George** | `fixtureserve` on `:7717`, all-kinds fixtures stable |

### Checkpoints

| ID | Show | Real/Fake | Tag |
|----|------|-----------|-----|
| CP1.0 | `/sidecar` opens/closes | shell OK empty | `cp-P1-slash-shell` |
| CP1.1 | 6 fixture kinds + AI tags | HTTP fixtures | `cp-P1-fixture-cards` |
| CP1.2 | Type → freeze | real UX | `cp-P1-freeze` |
| **CP1.3 gate** | full path stage laptop | fixtures | `cp-P1-done` |
| CP1-fallback | peek/web if TUI slips | fixtures | `cp-P1-fallback-peek` |

**Pitch:** “`/sidecar` — feed beside the agent; freezes when you type.”

---

## 6. Phase 2 — Build-in-public (user X)

### Build
| Who | Work |
|-----|------|
| **George** | `filter`, diff→draft card, OAuth1 `POST /2/tweets` or **labeled mock**, `approve_post` |
| **Abi** | dual-pane confirm (diff \| text), approve/dismiss in `/sidecar`; optional `/sidecar post` |

### Checkpoints

| ID | Show | Tag |
|----|------|-----|
| CP2.0 | draft-post card from dirty repo | `cp-P2-draft-card` |
| CP2.1 | confirm UI + dismiss | `cp-P2-confirm` |
| CP2.2 | planted secret redacted | `cp-P2-filter` |
| **CP2.3 gate** | approve → live X **or** “posted (demo mock)” | `cp-P2-done` |

**Pitch:** “Approve in the pane — link-free build-in-public on your X.”  
**Abort:** mock passes gate if labeled; don’t block on R+W token.

---

## 7. Phase 3 — Jam v0 (context only)

### Product
- Host: brain + `JAM_ID` + `JAM_TOKEN`  
- Members: join + `POST` **ContextEnvelope** (no file bodies)  
- **Room cards** `plane=room`, chrome **JAM · origin**  
- Meta: members + latest contexts  
- Freeze still **local** per user  
- LAN first (`JAM_HOST=http://ip:7717`)

### ContextEnvelope
```json
{
  "origin": "abi",
  "ts": 0,
  "mode": "waiting|active",
  "context": "one-line summary",
  "topic_keys": ["e0308", "ratatui"],
  "branch": "sidecar-main",
  "sha": "abc1234",
  "chat": "optional note",
  "dirty_summary": "3 files +40/-12"
}
```

### Build
| Who | Work |
|-----|------|
| **George** | join/context/meta routes, room cards, token auth, rate limit, ingress filter |
| **Abi** | member strip, JAM chrome, compose→POST; or George CLI `jam_publish.py` fallback |

### Checkpoints

| ID | Show | Tag |
|----|------|-----|
| CP3.0 | host in meta | `cp-P3-host` |
| CP3.1 | two origins | `cp-P3-two-members` |
| CP3.2 | JAM room card in `/sidecar` | `cp-P3-room-card` |
| **CP3.3 gate** | two-machine context room | `cp-P3-done` |

**Pitch:** “Same `/sidecar` — now a room; contexts sync.”  
**Not yet:** X search merge, sandbox, public tunnel required.

---

## 8. Phase 4 — Full Code Jam

### Pipeline
```
union(member contexts + chat)
  → topic_keys + context_union
  → X search/recent (+ optional x_search bg)
  → rank → curator (pull|digest|hold)
  → plane=research cards on shared feed
  → lurker RO web (+ LAN or cloudflared)
```

| Step | What | Priority |
|------|------|----------|
| 4a | Union + X research cards | MUST for “full jam” |
| 4b | Digest + caps + dedup | TARGET |
| 4c | Lurker + counter + tunnel/LAN | TARGET |
| 4d | A1 same git remote in meta | NICE |
| 4e | A2 Docker/code-server sandbox | STRETCH (kill if >45m net pain) |

### Checkpoints

| ID | Show | Tag |
|----|------|-----|
| CP4.0 | union one-liner | `cp-P4-union` |
| CP4.1 | live research card + source_url | `cp-P4-x-card` |
| CP4.2 | room + research + freeze | `cp-P4-mixed` |
| CP4.3 | lurker second device | `cp-P4-lurker` |
| **CP4.4 gate** | full demo script | `cp-P4-done` |
| CP4.s | sandbox | `cp-P4-sandbox` |

**Pitch:** “Room context steers X research; audience watches — Spaces for building.”

---

## 9. Architecture

```
                 ┌─────────────────────────────────────┐
  /sidecar TUI   │  ActiveView::Sidecar (Abi / fork)   │
  (or peek/web)  │  freeze local · plane chrome          │
                 └──────────────────▲────────────────────┘
                                    │ GET /state /cards
                                    │ POST /action
                                    │ POST /jam/... (P3+)
                 ┌──────────────────┴────────────────────┐
                 │  Brain :7717 (George / Python Flask)    │
                 │  CardStore · filter · draft · jam · X   │
                 └──────────────────┬────────────────────┘
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
              X search/recent   xAI (draft/     X POST tweet
              (research)        digest)         (approve only)
```

**Cloudflare tunnel:** optional transport so lurkers/off-LAN hit `:7717` read-only. Not the product. Prefer LAN on stage Wi‑Fi first. **Never** expose unauthenticated `approve_post` on a public URL.

---

## 10. HTTP contract (`contract/` — additive only)

### Solo v1 (exists — do not break)

```
GET  /state              → { mode: "waiting"|"active", context }
GET  /cards?since=<seq>  → { cards: [Card,...] }   # seq ↑
POST /action             → { card_id, action: save|dismiss|approve_post }
GET  /briefing.wav       → 503 until voice (cut OK)
```

### Card

```
id, seq, kind: real|digest|render|draft-post|voice-briefing
title, body, media_url, source_url
generated, justification, diff_context
# additive optional:
plane: room|research|local|draft-post
jam_origin: "abi"|null
topic_keys: []
```

`generated:true` ⇒ visible **AI-generated** tag always.

### Jam (Phase 3+)

```
POST /jam/:id/join       { origin, token }
POST /jam/:id/context    ContextEnvelope
GET  /jam/:id/meta       { members, context_union, topic_keys, repo_url? }
GET  /jam/:id/feed?since=
# When JAM_ID set, GET /cards MAY alias jam feed (TUI drop-in)
```

VERSION bump + both ack. New fixtures file for jam — don’t edit v1 fixtures in place.

---

## 11. API primitives (implement against these)

| Need | Primitive |
|------|-----------|
| Context | git status/diff stat → one-liner + topic_keys (± grok compress) |
| X research | `GET /2/tweets/search/recent` + media expansions (workhorse) |
| X enrich | xAI `POST /v1/responses` + `tools:[{type:x_search}]` (slow bg; `live_search` is 410) |
| Rank | x-algorithm **shape** + public_metrics proxies (honest claim) |
| Curator | pull / digest / HOLD (± render later) |
| Post | OAuth1 `POST /2/tweets`, **link-free**; R+W token or mock |
| Safety | one filter: gitignore-aware + key/entropy; ingress + approve |

Details: `docs/PRIMITIVES.md`.

---

## 12. Repo layout (target)

```
docs/SIDECAR.md              ← THIS FILE (SoT)
docs/PRIMITIVES.md           ← API ground truth (keep)
docs/george-plan.md          ← historical; defer to SIDECAR.md
contract/schema.md           ← wire + additive jam notes
contract/fixtures/…
sidecar/brain/               ← George
  server/ distill/ feed/ xsearch/ post/ jam/
sidecar/fixtureserve.py peek.py web/ jam_publish.py
sidecar/tests/
crates/.../xai-grok-pager/   ← Abi: slash + ActiveView::Sidecar
```

---

## 13. Implementation order (first principles → phases)

```
P1 bundle:  slash OpenSidecar · view shell · fixtureserve · freeze · chrome
P2 bundle:  filter · draft card · confirm UI · approve_post · X/mock
P3 bundle:  jam auth · context store · room cards · publisher · JAM chrome
P4 bundle:  union · xsearch · rank/curator · /cards alias · lurker
            then A1 meta / A2 sandbox if time
```

**Start at CP1.0.** Not Docker, not Imagine, not jam relay before TUI.

### Within-phase unit accept (short)

| Phase | Accept sketch |
|-------|----------------|
| 1 | `/sidecar` + fixtures cards + freeze |
| 2 | approve path + filter test green |
| 3 | two origins in meta + room card |
| 4a | research card with source_url from live search |

`make test && make contract` stay green; solo M0 path when `JAM_ID` unset.

---

## 14. Timeline (~9h) + gates

| Wall | Phase | Must leave with |
|------|-------|-----------------|
| 0–2.5h | 1 | **CP1.3** or fallback peek |
| 2.5–4.5h | 2 | **CP2.3** |
| 4.5–6.5h | 3 | **CP3.3** |
| 6.5–8h | 4a–c | **CP4.1** min; CP4.4 ideal |
| 8–9h | rehearse off tag + record | fallback video |

---

## 15. Cut ladder

1. A2 sandbox  
2. A1 git chrome  
3. Tunnel (LAN OK)  
4. Digests / Imagine render / voice  
5. Entire Phase 4 → ship CP3.3  
6. Phase 3 → ship CP2.3  
7. Live X → mock at CP2.3  
8. **Never cut:** `/sidecar` (or labeled peek) + freeze + something on screen  

---

## 16. Always-demo matrix

| Last green | Demo | Run |
|------------|------|-----|
| CP1.3 | Native pane | fixtureserve · grok · `/sidecar` · type |
| CP2.3 | + ship to X | brain · draft approve |
| CP3.3 | + context room | host+member · JAM cards |
| CP4.2+ | full jam | + X research · lurker |

---

## 17. Full demo script (only CP4.4)

1. `/sidecar` opens feed  
2. Agent grinds → wake; type → freeze  
3. Draft → approve → X/mock  
4. Peer context → **JAM · abi** card  
5. Research cards on room topics  
6. Lurker QR + counter  

Stop early at any `cp-*-done` and pitch that depth honestly.

---

## 18. Env names only

`SIDECAR=1`, `SIDECAR_REPO`, `SIDECAR_ADDR`, `SIDECAR_PORT=7717`,  
`JAM_ID`, `JAM_TOKEN`, `JAM_HOST`, `JAM_ORIGIN`,  
`XAI_API_KEY`, `XAI_BASE_URL`, `XAI_MODEL`,  
`X_BEARER_TOKEN`, `X_CONSUMER_KEY`, `X_CONSUMER_SECRET`, `X_ACCESS_TOKEN`, `X_ACCESS_TOKEN_SECRET`,  
`TUNNEL=1` optional.

---

## 19. Non-goals tonight

CRDT multiplayer IDE · Spaces audio · For You API · auto-post research feed to X · code-gen apply cards · compiling x-algorithm Rust · day-recap spam threads · plugin-as-pane fantasy.

---

## 20. Pitch lines

- “Everyone tabs to X while the agent works — `/sidecar` keeps it in the tool, with a governor.”  
- “Build-in-public from a real diff — human approve, not a bot spray.”  
- “Code Jam: room context + X research; feed is how the room sees the work.”  
- “Thin fork: one slash + one view, feature-flagged — not a spoof app.”  

---

## 21. Immediate next (after push + pull)

| Who | First move |
|-----|------------|
| **Abi** | CP1.0 — `/sidecar` opens empty Sidecar view |
| **George** | Confirm fixtureserve + fixtures for CP1.1; don’t block on jam |
| **Both** | Pull `docs/SIDECAR.md`; ack phases; tag every CP |

---

## 22. Related docs

| Doc | Role |
|-----|------|
| **`docs/SIDECAR.md` (this)** | Build SoT |
| `docs/PRIMITIVES.md` | X/xAI API ground truth |
| `contract/*` | Wire protocol + fixtures |
| `docs/CODE-JAM.md`, `docs/george-plan.md` | Historical (superseded headers) |
| `build/*` | Optional lane boards — align to phases when coding |

---

*End consolidated plan. Implement only from this file + PRIMITIVES + contract.*
'''
