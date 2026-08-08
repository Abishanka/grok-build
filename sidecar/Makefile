PY := .venv/bin/python
REPO ?= $(abspath ..)

setup:
	python3 -m venv .venv && .venv/bin/pip install -q -r requirements.txt

run:
	SIDECAR_REPO=$(REPO) $(PY) -m brain

peek:
	$(PY) peek.py --watch 2

fixtureserve:
	$(PY) fixtureserve.py

test:
	$(PY) -m pytest tests/ -q

contract:
	$(PY) -m pytest tests/test_contract.py -q

smoke:
	./scripts/smoke.sh
