> **Superseded by [`docs/SIDECAR.md`](SIDECAR.md)** — single SoT for phases, `/sidecar`, checkpoints. Keep this file for history only.

# CODE JAM / SIDECAR — shared plan (George + Abi)

**Branch:** `sidecar-main` · **Repo:** https://github.com/Abishanka/grok-build  
**Updated:** prioritization = TUI → build-in-public → context jam → full jam · entry = `/sidecar`

# SIDECAR — build plan (slash entry + checkpoint spine)

**This file is the shared SoT** for George + Abi on `sidecar-main`.  
**Branch:** `sidecar-main` · local first · creds in zshrc  
**Owners:** **Abi** = TUI/pager · **George** = brain/API/X/jam

---

## 0. Priority law (unchanged spine)

```
PHASE 1  TUI works
PHASE 2  Build-in-public → user X account
PHASE 3  Jam = share context only
PHASE 4  Full Code Jam (union + X research feed + audience)
```

Do not start Phase N+1 until Phase N has a **checkpoint tag** and a **30-second show path**.

---

## 1. How you open Sidecar in grok-build (not a hotkey)

### What grok-build actually allows

| Mechanism | Can open a native feed pane? | Notes |
|-----------|------------------------------|--------|
| **Pager slash builtin** (e.g. `/dashboard`) | **Yes** | Returns `Action::…` → switches `ActiveView`. **This is the path.** |
| **Plugin `commands/` or skill** | **No UI pane** | Adds slash text/skills/hooks/MCP only. Cannot inject `ActiveView`. |
| **Hooks** | No pane | Good later for auto-pushing context into jam (Phase 3+) |
| **Raw hotkey only** | Possible but wrong default | Easy to miss; clashes with host bindings |

**Precedent in-tree:**  
`/dashboard` → `DashboardCommand` → `Action::OpenDashboard` → `ActiveView::AgentDashboard`  
(`crates/codegen/xai-grok-pager/src/slash/commands/dashboard.rs`)

### Locked entry UX

| Command | Behavior |
|---------|----------|
| **`/sidecar`** | Toggle/open `ActiveView::Sidecar` (feed). Alias: `/jam` optional later |
| **`/sidecar post`** (Phase 2+) | Jump to draft-post confirm if a draft exists, else toast “no draft” |
| **`/sidecar leave`** | Back to agent view (or Esc — document both) |

**Feature flag:** `SIDECAR=1` or `[sidecar].enabled = true` (mirror dashboard’s enable gate) so the fork stays thin and pitchable.

**Plugin role (secondary, not the pane):**
- Optional plugin package: hooks that POST context, skill docs “how to jam”, `jam_publish` helper script  
- Install story for Most Users later  
- **Does not replace** the forked `/sidecar` builtin

**Fallback if fork slips:** dual-pane terminal + `peek`/web still demoable, opened by running brain — label honestly “Sidecar peek” not grok-build native.

---

## 2. Checkpoint doctrine (“always something to show”)

Every checkpoint must answer:

1. **What do we run?** (≤3 commands)  
2. **What does a judge see in 30s?**  
3. **What is fake vs real?**  
4. **Git tag** on local `sidecar-main`: `cp-P{phase}-{HHMM}-{note}`  
5. **Who can demo it alone?** (Abi / George / either)

**Rule:** No merge into “main demo” without a checkpoint. If a phase fails, **roll demo back to last green checkpoint** — never half-broken HEAD.

**Hard checkpoint reviews (wall clock):** end of each phase + optional mid-phase mini-CPs below.

---

## 3. Phase 1 — TUI works (`/sidecar`)

### Build
- Abi: `ActiveView::Sidecar` + **`/sidecar` slash command** (clone dashboard pattern) + scroll + kind chrome + AI tags + freeze-on-type badge + leave  
- George: `make fixtureserve` rock-solid on `:7717`; fixtures all card kinds  

### Checkpoints

| ID | When | Show path | Fake/Real | Tag example |
|----|------|-----------|-----------|-------------|
| **CP1.0** | Slash wired, empty/placeholder view | `/sidecar` opens pane, Esc/leave returns | Fake body OK | `cp-P1-slash-shell` |
| **CP1.1** | Fixture cards render | fixtureserve + `/sidecar` shows 6 kinds, AI tags | Real HTTP fixtures | `cp-P1-fixture-cards` |
| **CP1.2** | Freeze governor | Type in agent → FREEZE badge on feed | Real local UX | `cp-P1-freeze` |
| **CP1.3** | **Phase 1 gate** | Full path above on stage laptop | Fixtures | `cp-P1-done` |

**30s pitch at CP1.3:** “`/sidecar` — feed beside the agent; freezes when you type.”

**Abort:** If CP1.1 missing at T+2.5h → CP1-fallback = peek/web dual-pane; still tag `cp-P1-fallback-peek`.

---

## 4. Phase 2 — Build-in-public (user X)

### Build
- George: filter, diff→draft card, OAuth1 `POST /2/tweets` or mock, `approve_post`  
- Abi: draft confirm dual-pane; `/sidecar` shows draft-post; approve/dismiss actions  

### Checkpoints

| ID | When | Show path | Fake/Real | Tag |
|----|------|-----------|-----------|-----|
| **CP2.0** | Draft card only | Dirty repo → draft-post in `/sidecar` (no post yet) | Diff real; text may be templated | `cp-P2-draft-card` |
| **CP2.1** | Confirm UI | Side-by-side diff \| text; dismiss works | Real UI | `cp-P2-confirm` |
| **CP2.2** | Filter beat | Planted `sk-…` in diff redacted in draft | Real filter | `cp-P2-filter` |
| **CP2.3** | **Phase 2 gate** | Approve → tweet on X **or** banner “posted (demo mock)” | Live if R+W else mock | `cp-P2-done` |

**30s pitch at CP2.3:** “Approve from the pane — ships link-free build-in-public on *your* X account.”

**Abort:** Mock counts as gate pass if labeled; do not block Phase 3 on R+W token.

---

## 5. Phase 3 — Jam v0 (context share only)

### Build
- George: join/context/meta + room cards  
- Abi: member strip, JAM chrome, compose context/chat → POST; still `/sidecar`  
- Optional plugin/hook later to auto-post context — not required for gate  

### Checkpoints

| ID | When | Show path | Fake/Real | Tag |
|----|------|-----------|-----------|-----|
| **CP3.0** | Solo join | Host meta shows host origin | Real relay | `cp-P3-host` |
| **CP3.1** | Second origin | Abi (or CLI publisher) → second member on meta | Real LAN | `cp-P3-two-members` |
| **CP3.2** | Room card | Context/chat → **JAM · abi** card in `/sidecar` | Real | `cp-P3-room-card` |
| **CP3.3** | **Phase 3 gate** | Two laptops: shared room activity visible both sides (or host TUI + member CLI) | Real context; no X search required | `cp-P3-done` |

**30s pitch at CP3.3:** “Same `/sidecar` — now a room; contexts sync; JAM vs local chrome.”

---

## 6. Phase 4 — Full Code Jam

### Build
- Union distill + X search/recent → research cards on same feed  
- Lurker RO web + LAN/tunnel  
- Optional J1 git meta / J2 sandbox  

### Checkpoints

| ID | When | Show path | Fake/Real | Tag |
|----|------|-----------|-----------|-----|
| **CP4.0** | Union context | `/state` or meta shows merged one-liner | Real | `cp-P4-union` |
| **CP4.1** | First X card | `plane=research` + source_url in `/sidecar` | **Live X** | `cp-P4-x-card` |
| **CP4.2** | Mixed feed | Room + research + freeze + optional digest | Live | `cp-P4-mixed` |
| **CP4.3** | Audience | Second device lurker URL | Live RO | `cp-P4-lurker` |
| **CP4.4** | **Phase 4 gate** | Full demo script below | Live | `cp-P4-done` |
| **CP4.s** | Stretch sandbox | code-server both in same box | Optional | `cp-P4-sandbox` |

**30s pitch at CP4.4:** “Room context steers X research; audience watches the feed — Spaces-shaped, build-shaped.”

---

## 7. Always-demo matrix (what you show if you stop now)

| Last green CP | Demo title | Commands |
|---------------|------------|----------|
| CP1.3 | Native Sidecar pane | fixtureserve; grok; `/sidecar`; type to freeze |
| CP2.3 | Pane + ship to X | brain; `/sidecar`; approve draft |
| CP3.3 | Context room | host+member; `/sidecar` JAM cards |
| CP4.2+ | Full Code Jam | + X research + lurker |

Keep a **one-pager runbook** (later in repo): `docs/DEMO-CHECKPOINTS.md` listing tags + commands — create when implementing.

---

## 8. Timeline with gates

| Wall | Goal | Must leave with |
|------|------|-----------------|
| 0–2.5h | Phase 1 | **CP1.3** or CP1-fallback |
| 2.5–4.5h | Phase 2 | **CP2.3** |
| 4.5–6.5h | Phase 3 | **CP3.3** |
| 6.5–8h | Phase 4a–c | **CP4.1** minimum; CP4.4 if possible |
| 8–9h | Rehearse off latest `cp-*-done` tag; record | fallback video |

---

## 9. Cut ladder

1. CP4.s sandbox  
2. CP4.3 tunnel (LAN OK)  
3. Digests  
4. Entire Phase 4 (ship CP3.3)  
5. Phase 3 (ship CP2.3)  
6. Live X (mock at CP2.3)  
7. Never cut: `/sidecar` open path + something on screen (CP1.*)  

---

## 10. Implementation note for Abi (slash, not plugin pane)

```
// Pattern (conceptual) — mirror dashboard.rs
/sidecar  → Action::OpenSidecar → ActiveView::Sidecar
// register in slash registry; feature-flag like dashboard
// plugins: optional hooks/skills ONLY
```

---

## 11. Immediate next actions

1. **Abi:** CP1.0 — `/sidecar` opens empty Sidecar view  
2. **George:** fixtureserve + confirm fixture JSON for CP1.1  
3. After each CP: tag + 30s dry run before continuing  
4. Next: Abi CP1.0 `/sidecar` shell; George fixtureserve for CP1.1

---

## 12. Full demo script (only if CP4.4)

1. `/sidecar` opens feed  
2. Agent grinds → wake; type → freeze  
3. Draft → approve → X/mock  
4. Second user context → JAM card  
5. X research cards on room topics  
6. Lurker QR  

If only CP2.3: steps 1–3. If CP3.3: 1–4. Always end on a **tagged** build.
'''


---

## Appendix A — Product one-liner & tiers (context)

**Code Jam** = shared session where context (+ later workspace) drives a **live Sidecar feed** (room perception) with **X search in the loop**.

| Tier | Meaning | Phase |
|------|---------|-------|
| Context jam | Relay envelopes + room cards | Phase 3 |
| Full jam | Union + X research + audience | Phase 4 |
| A1 same git remote | Convention in meta | Phase 4 nice |
| A2 sandbox | Docker/code-server | Phase 4 stretch |

**Never cut:** `/sidecar` show path → freeze → (P2) draft/approve → plane demarcation when jam exists → secret filter on egress.

**Non-goals tonight:** CRDT IDE, Spaces audio, auto-post full feed to X, For You API, day-recap threads.

---

## Appendix B — Contract sketch (additive)

Solo `:7717` remains. Jam adds when Phase 3+:

```
POST /jam/:id/join          { origin, token }
POST /jam/:id/context       ContextEnvelope
GET  /jam/:id/meta
GET  /jam/:id/feed?since=
GET  /cards                 # alias jam feed when JAM_ID set
POST /action                save|dismiss|approve_post
```

Card optional fields: `plane` (`room|research|local|draft-post`), `jam_origin`, `topic_keys`.  
Bump `contract/VERSION` only with both humans. Fixtures v1 not edited in place — add `cards_v1_jam.json`.

ContextEnvelope (no file bodies):
`origin, ts, mode, context, topic_keys[], branch?, sha?, chat?, dirty_summary?`

---

## Appendix C — Split

| | Abi (Rust pager) | George (`sidecar/`) |
|--|------------------|---------------------|
| P1 | `/sidecar`, ActiveView, chrome, freeze | fixtureserve |
| P2 | draft confirm UI, actions | filter, draft, X client, approve |
| P3 | JAM chrome, members, compose | jam relay, room cards |
| P4 | polish | union, xsearch, lurker, tunnel |

Anti-collision: Abi = crates tree; George = sidecar/docs/contract additive.

---

## Appendix D — First-principles order (within phases)

```
Phase 1: slash + view + fixtures + freeze
Phase 2: filter → draft card → confirm → approve_post → X/mock
Phase 3: auth/join → context POST → meta → room cards → publisher
Phase 4: union → X recent → rank/curator → research cards → lurker
         then J1 meta / J2 sandbox if time
```

Coding starts at **CP1.0**, not Docker/Imagine/jam relay.

---

## Appendix E — Env (names only)

`JAM_ID`, `JAM_TOKEN`, `JAM_HOST`, `JAM_ORIGIN`, `SIDECAR_REPO`, `SIDECAR=1`, `XAI_*`, `X_*` (OAuth1 for post), `TUNNEL=1`.  
From zshrc/shell; never commit `.env`.

---

## Appendix F — Related docs

| Doc | Role |
|------|------|
| `docs/CODE-JAM.md` (this) | Shared build SoT |
| `docs/george-plan.md` | Earlier solo-brain ops (superseded on priority by this file) |
| `docs/PRIMITIVES.md` | X/xAI API ground truth |
| `contract/*` | Wire protocol |
| `build/LANE-*.md` | Optional unit boards — align to phases when coding |

---

## Appendix G — Demo scripts by checkpoint

**CP1.3:** fixtureserve · grok · `/sidecar` · type to freeze  
**CP2.3:** + draft approve → X/mock  
**CP3.3:** + second origin context → JAM card  
**CP4.4:** + X research + lurker QR  

Always demo from latest `cp-*-done` tag, not dirty HEAD.
