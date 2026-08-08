# Contract v1 — localhost:7717 (FROZEN: changes need both humans + VERSION bump)

GET /state -> {"mode":"waiting|active","context":"one-line summary"}
GET /cards?since=<seq> -> {"cards":[Card,...]}   (seq strictly increasing)
POST /action {"card_id":"c1","action":"save|dismiss|approve_post"} -> {"ok":true}
GET /briefing.wav -> audio/wav (503 until voice ships)

Card:
  id: str            seq: int
  kind: "real"|"digest"|"render"|"draft-post"|"voice-briefing"
  title: str         body: str
  media_url: str|""  source_url: str|""
  generated: bool    justification: str|""
  diff_context: str|""   (draft-post only: literal diff lines)

Rules: every generated:true card gets a visible AI-generated tag in the
TUI. v1 fixture files are never edited in place — supersede as v2.
