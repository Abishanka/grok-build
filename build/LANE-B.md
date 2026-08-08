# LANE B — xsearch + feed (owner: lane-b session ONLY)

- [ ] B1: search/recent client + media expansion join (author, photos url,
      video preview) from raw response
      files: brain/xsearch/
      accept: cd sidecar && python -m pytest tests/test_xsearch.py
      demo-visible: yes (first real X card)
- [ ] B2: prefetch loop + cache (since_id, 24h-dedup-aware, only during
      mode=waiting), capped
      files: brain/xsearch/, brain/feed/
      needs: B1
      accept: cd sidecar && python -m pytest tests/test_prefetch.py
      demo-visible: yes
- [ ] B3: ranker — x-algorithm shape (Σw·p proxies from public_metrics ->
      author-diversity decay -> OON x0.75 -> age cutoff)
      files: brain/feed/
      accept: cd sidecar && python -m pytest tests/test_ranker.py
      demo-visible: yes (ordering)
- [ ] B4: digest cards — grok-4.5 condenses top-K same-topic posts,
      generated:true
      files: brain/feed/
      needs: B1
      accept: cd sidecar && python -m pytest tests/test_contract.py
      demo-visible: yes
- [ ] B5: curator loop — pull/digest/render/HOLD decisions + justification
      strings; junk context => HOLD
      files: brain/feed/
      needs: B2,B3
      accept: cd sidecar && python -m pytest tests/test_curator.py
      demo-visible: yes
- [ ] B6 (stretch): filtered-stream live mode, rules rewritten on A4 events
      files: brain/xsearch/
      needs: B5,A4
      accept: manual — stream card lands <30s after seeded post
- [ ] B7 (stretch): liked_tweets taste vector nudges ranker weights
      files: brain/feed/
      needs: B3
      accept: cd sidecar && python -m pytest tests/test_ranker.py::test_taste
