# Sidecar services

## Code Jam (primary) — Go `net/http`

```bash
cd sidecar
make jam                 # :7720  JAM_ID=demo JAM_TOKEN=dev
make smoke-jam           # API smoke (server must be up)
make publish             # push one context as member
make tui                 # interactive jam-tui (real multi-user client)
make proof-tui           # two jam-tui --once clients share the room
open http://127.0.0.1:7720/jam.html?jam=demo
```

Binary: `make jam-build` → `bin/jamd` · `make tui-build` → `bin/jam-tui`  
TUI wiring notes: [`../docs/TUI-WIRING.md`](../docs/TUI-WIRING.md)

| Env | Purpose |
|-----|---------|
| `JAM_ID` | session id (default `demo`) |
| `JAM_TOKEN` | write auth (empty = open dev) |
| `JAM_PUBLIC_RO=1` | block `approve_post` on public edge (set `0` for local approve demos) |
| `X_BEARER_TOKEN` | in-process X `search/recent` research worker (plane=research cards) |
| `SIDECAR_REPO` / `JAM_REPO_PATH` | git root for draft-post cards |
| `JAM_POST_MOCK=1` | force mock X post on approve (no live tweet) |
| `X_CONSUMER_KEY` + secret + access token pair | live OAuth1 `POST /2/tweets` when not mock |
| `SUPABASE_URL` + `SUPABASE_SERVICE_ROLE_KEY` | optional durable members/cards |
| `DATABASE_URL` | optional Postgres source of truth |
| `REDIS_URL` / `JAM_REDIS_URL` | optional Redis pub/sub for multi-node SSE (`jam:{id}:events`) |
| `SANDBOX_*` | docker code-server control |

### Event bus (Redis / in-process)

Default is **in-process memory pub/sub** — SSE is push-driven on the same node.
Set `REDIS_URL=redis://127.0.0.1:6379` to fan card/tip/sandbox events across jamd
processes (channel `jam:{JAM_ID}:events`). `/health` reports `"bus":"memory"|"redis"`.

```bash
make redis-up                          # docker compose profile redis
REDIS_URL=redis://127.0.0.1:6379 make jam
make smoke-bus                         # health.bus + live SSE push
EXPECT_BUS=redis make smoke-bus
```

### Built into jamd (Phases 2–4)

| Feature | How |
|---------|-----|
| Room context | `POST /jam/:id/context` → room cards + tip log |
| X research | background worker on union `topic_keys` → `plane=research` |
| Draft-post | polls dirty git → `kind=draft-post` card with `diff_context` |
| Approve | `POST /action` `approve_post` → mock or live X (blocked if `JAM_PUBLIC_RO=1`) |
| Sandbox | `POST /jam/:id/sandbox/start` → docker code-server |
| Lurker UI | `GET /jam.html?jam=demo` (+ `&token=dev` for member controls) |

### Sandbox (A2)

Not Modal/Firecracker. **Docker + `codercom/code-server`** via jam API:

```bash
# jamd running with docker.sock (compose) or local docker
curl -X POST http://127.0.0.1:7720/jam/demo/sandbox/start -H "X-Jam-Token: dev"
curl http://127.0.0.1:7720/jam/demo/sandbox
# → {status, url}  e.g. http://127.0.0.1:8080
make smoke-sandbox   # live E2E: start → HTTP probe code-server → stop
```

Or compose profile: `docker compose --profile sandbox up sandbox`

### Supabase (Abi)

SQL: `supabase/migrations/001_jam.sql`  
Then set Railway/local env `SUPABASE_URL` + `SUPABASE_SERVICE_ROLE_KEY`.  
Without them, jam is pure in-memory (fine for local).


### Railway (live) — off localhost

Project: [disciplined-friendship](https://railway.com/project/a23cd1a7-4043-4c26-b3c5-1796aaace149)

| Service | Role | URL |
|---------|------|-----|
| **jamd** | Public edge (`GET /cards`, lurker, jam API) | https://jamd-production.up.railway.app |
| **braind** | Private worker → posts research cards to jamd | no public domain |

```
TUI / browser / peek
        │  HTTPS
        ▼
   jamd (public)  ◄── private net ──  braind (worker)
   :7717 /cards                         JAM_HOST=http://jamd.railway.internal:7717
```

- Health: https://jamd-production.up.railway.app/health  
- Lurker: https://jamd-production.up.railway.app/jam.html?jam=demo  
- Member: https://jamd-production.up.railway.app/jam.html?jam=demo&token=$JAM_TOKEN  
- Contract: `GET /cards?since=0` · `GET /state` · `POST /jam/demo/context`  
- `JAM_PUBLIC_RO=1` → `approve_post` blocked on the public edge (by design)

**Deploy both (from `sidecar/`):**
```bash
./scripts/deploy-railway.sh both     # jamd from sidecar/, braind from braind/
./scripts/smoke-prod.sh              # public e2e (room + research)
# or: make deploy-prod && make smoke-prod
```

`braind` env is wired to jamd via Railway refs (`JAM_HOST`, `JAM_TOKEN`, `X_*`, `XAI_*`).  
Research cards also come from jamd’s in-process X worker (OAuth2 user token after Connect X).  
If braind logs `x search 401`, rotate `X_BEARER_TOKEN` (app-only) on jamd — braind inherits it.

**Do not** deploy the empty `feeder-api` service in the grokathon Railway project for this path.

### Live X post — OAuth 2.0 (PKCE)

Preferred path: **OAuth 2.0 user context** with PKCE. jamd hosts the callback.

#### Callback URL (paste into X developer portal)

```
https://jamd-production.up.railway.app/oauth/x/callback
```

Also add local if you login against a laptop jamd:

```
http://127.0.0.1:7720/oauth/x/callback
```

Portal settings (app `grusu-hackathon-26` or yours):
- **App permissions:** Read and write  
- **Type of App:** Web App, Automated App or Bot  
- **Callback URI:** the Railway URL above  
- **Website URL:** `https://jamd-production.up.railway.app/`  

Then copy **OAuth 2.0 Client ID** and **Client Secret** into env:

| Env | Value |
|-----|--------|
| `X_OAUTH2_CLIENT_ID` | Client ID |
| `X_OAUTH2_CLIENT_SECRET` | Client Secret |
| `X_OAUTH2_REDIRECT_URI` | `https://jamd-production.up.railway.app/oauth/x/callback` |
| `JAM_PUBLIC_URL` | `https://jamd-production.up.railway.app` (used if REDIRECT unset) |

#### Login + post

1. Open https://jamd-production.up.railway.app/oauth/x/login (or **Connect X** on jam.html)  
2. Approve scopes `tweet.read tweet.write users.read offline.access`  
3. Land on callback → tokens stored → approve draft-posts  

```bash
curl -s https://jamd-production.up.railway.app/oauth/x/status
# {"enabled":true,"connected":true,...}
```

Legacy OAuth 1.0a (`X_CONSUMER_*` + `X_ACCESS_TOKEN*`) still works if OAuth2 client id is unset.

---

## braind — Phase 4 research spine

Dedicated Go worker (not the optional Python Flask brain). Pipeline:

```
jam tips/meta (files · errors · goals · topic_keys)
  → distill + scrub secrets
  → BuildQuery
  → X GET /2/tweets/search/recent   (workhorse)
  → optional xAI chat digest
  → optional bg xAI POST /v1/responses + tools:[{type:x_search}]
  → rank / dedupe → POST /jam/:id/cards  (plane=research)
```

```bash
make jam                 # terminal 1 — jamd :7720
make braind              # terminal 2 — worker
# or: make jam-dev       # both
make test-brain          # go test ./... in braind/
```

| Env | Purpose |
|-----|---------|
| `JAM_HOST` | jamd base (default `http://127.0.0.1:7720`) |
| `JAM_ID` / `JAM_TOKEN` | session + write auth for cards |
| `X_BEARER_TOKEN` | X recent-search (missing → **HOLD**, no filler cards) |
| `XAI_API_KEY` | optional chat digest |
| `BRAIN_ENABLE_XAI_SEARCH=1` | background `x_search` via Responses API (off by default) |
| `BRAIN_XAI_SEARCH_TIMEOUT` | default `25s` — fail-open on timeout |
| `BRAIN_MAX_CARDS` | cap per recent-search run (default 5) |
| `BRAIN_DEBOUNCE` / `BRAIN_POLL_INTERVAL` | tip coalesce (10s / 3s) |

Curator **HOLD**s when bearer missing, search errors, or no matches. Empty beats filler.
`live_search` is dead (410) — only `x_search`.

**Railway:** braind is a worker; point `JAM_HOST` at jamd (local or Railway jamd). Prefer local green spine first. Do not revive the old Python `feeder-api` as the Sidecar brain.
