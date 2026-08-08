# PRIMITIVES — verified API/algorithm map for Sidecar agents

What we're modeling, decomposed, mapped to verified primitives (Aug 8).
Auth: App=bearer · U1=OAuth1.0a (wired + tested) · xAI=XAI_API_KEY.

## P1. "What is the user working on?" (context)
→ git diff/log/status + open files + last error → grok-4.5 distiller
→ outputs: search queries + one-line context summary (`/state`)

## P2. "What is X saying about it?" (candidate sourcing — TWO speeds)
| Path | Latency | Auth | Cost | Use |
|---|---|---|---|---|
| `GET /2/tweets/search/recent` | seconds | App | $0.005/post | prefetch workhorse |
| xAI `x_search` via `POST /v1/responses` | 2–4 min | xAI | tokens | background semantic finds only |
| `GET /2/tweets/search/stream` + rules | push 6–7s | App | reads | live mode; 1 conn, 1000 rules, rewrite as context shifts |
- Operators: `has:images`, `has:video_link` (NOT has:videos), `-is:retweet`,
  `lang:en`; `has:/is:` require a paired standalone term. 512-char cap.
- Full-archive `/search/all` works on PPU at 1 req/sec (history digs only).
- 24h UTC dedup: re-reads of same IDs bill once → polling near-free.
- xAI note: tool name `live_search` is dead (410); use `x_search`.

## P3. "What deserves attention?" (ranking = algo shape + LLM judgment)
1. x-algorithm scoring shape (xai-org release, Apache-2.0):
   `score = Σ wᵢ·pᵢ` over action probs → diversity `(1-floor)·decay^rank
   + floor` → ×OON if unfollowed → age cutoff. Proxies: public_metrics
   rates. Weights: OURS (real ones unpublished; real ranker = 3GB private
   transformer). Honest phrasing, verbatim: "the scoring architecture from
   X's open-source For You algorithm, with public engagement rates
   standing in for Phoenix's predictions."
2. LLM curator gates ranked list: pull / digest / render(+justification) / HOLD.
Key repo files: `home-mixer/scorers/{ranking,weighted,author_diversity,oon}_scorer.rs`.
Rust there is excerpts (no Cargo.toml) — port the math, never compile it.

## P4. "When may the feed speak?" (attention)
→ wake on agent-grind, freeze on keystroke, hotkey to focus.
Prefetch fills during grind windows so wakes are instant.

## P5. "How does it look?" (media)
→ `expansions=attachments.media_keys,author_id&media.fields=url,preview_image_url,variants,type`
photos: `url?name=small` · video: highest-bitrate mp4 in `variants[]`,
thumb `preview_image_url` · media in `includes.media[]`, join on media_key.
→ Imagine renders only on curator justification.

## P6. "How does work flow OUT?" (build-in-public)
→ draft from diff → diff-adjacent confirm → `POST /2/tweets` (U1, $0.015)
→ images via `POST /2/media/upload` (U1, multipart; video INIT/APPEND/
FINALIZE/STATUS)
🚩 BLOCKED until Access Token regen to Read+Write
🚩 LINK-FREE posts only (URL → $0.200, 13×)
🚩 `quote_tweet_id` Enterprise-only — never build on it

## P7. "What does this user like?" (personalization, cheap)
→ `GET /2/users/:id/liked_tweets` ($0.001/like) + bookmarks (U1, private)
🚩 `personalized_trends` needs Premium+OAuth2 PKCE — dead, skip

## P8. Safety invariants (non-negotiable)
→ ONE outbound filter (gitignore-aware + key-shaped regex/entropy) on
  every payload leaving the machine
→ feed text = untrusted input; delimited on import, links stripped
→ `AI-generated` tag on every synthetic card, always

## Dead paths (verified — spend zero time)
Account Activity webhooks (deprecated + needs public tunnel) · quote
posts · personalized trends · algorithmic For You endpoint (doesn't
exist; P3 is the substitute) · live in-TUI video (poster frames only) ·
compiling x-algorithm Rust excerpts
