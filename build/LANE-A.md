# LANE A — server + distill (owner: lane-a session ONLY)

- [x] A1: M0 git-watcher serves real cards about this repo
      files: brain/distill/, brain/server/, brain/feed/ (store seed)
      accept: cd sidecar && make smoke
      demo-visible: yes — seeded by integrator in initial scaffold
- [ ] A2: /state wake heuristic (mode=waiting when repo active in last
      120s but no keystroke proxy; hardcode->heuristic)
      files: brain/server/, brain/distill/
      needs: A1
      accept: cd sidecar && python -m pytest tests/test_state.py
      demo-visible: yes (feed wakes/sleeps)
- [ ] A3: distiller v1 — git context -> grok-4.5 -> 2-3 search queries +
      one-line summary in /state.context
      files: brain/distill/
      needs: A1
      accept: cd sidecar && python -m pytest tests/test_distill.py
      demo-visible: yes (context line in pane header)
- [ ] A4: context-change detection (branch switch / new error) triggers
      re-query event for lane B prefetch
      files: brain/distill/
      needs: A3
      accept: cd sidecar && python -m pytest tests/test_distill.py::test_change_detect
      demo-visible: no
