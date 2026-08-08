# LANE C — gen + post (owner: lane-c session ONLY)

- [ ] C1: outbound secret filter (gitignore-aware + sk-/AKIA/entropy regex,
      redact before ANY payload)
      files: brain/post/
      accept: cd sidecar && python -m pytest tests/test_filter.py
      demo-visible: yes (live secret-catch beat)
- [ ] C2: draft-post generation from real diff; card carries verbatim
      diff_context; passes through C1
      files: brain/post/
      needs: C1
      accept: cd sidecar && python -m pytest tests/test_draft.py
      demo-visible: yes
- [ ] C3: X post client (OAuth1, link-free enforcement) + media upload
      🚩 BLOCKED on Access Token R+W regen
      files: brain/post/
      needs: C2
      accept: manual — approve card, read post back via API
      demo-visible: yes (build-in-public goes live)
- [ ] C4: public feed page + cloudflared tunnel + counter
      files: web/, scripts/tunnel.sh
      accept: curl the public URL, cards render
      demo-visible: yes (Most Users URL)
- [ ] C5: voice briefing — TTS summary on wake/focus -> /briefing.wav
      files: brain/gen/
      accept: cd sidecar && ./scripts/smoke.sh (briefing section 200)
      demo-visible: yes
- [ ] C6: Imagine render path (curator-justified only)
      files: brain/gen/
      needs: B5
      accept: cd sidecar && python -m pytest tests/test_contract.py
      demo-visible: yes
- [ ] C7 (stretch, go/no-go 7:30pm): ttyd read-only mirror per plan
      files: scripts/ttyd.sh
      accept: manual — second device renders text-card fallback
- [ ] C8 (stretch): day-recap thread from checkpoint tags
      files: brain/post/
      needs: C3
      accept: manual — approve + posted thread
