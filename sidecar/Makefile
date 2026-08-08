PY := .venv/bin/python
REPO ?= $(abspath ..)
JAMD := cd jamd && go run .
# Load repo .env when present (never print secrets)
-include $(REPO)/.env
export

setup:
	python3 -m venv .venv && .venv/bin/pip install -q -r requirements.txt

# Solo Python brain (optional; not Code Jam)
run:
	SIDECAR_REPO=$(REPO) $(PY) -m brain

# Primary Code Jam server — Go net/http (default :7720)
jam:
	cd jamd && SIDECAR_ADDR=0.0.0.0 SIDECAR_PORT=$${JAM_PORT:-7720} JAM_ID=$${JAM_ID:-demo} JAM_TOKEN=$${JAM_TOKEN:-dev} JAM_ORIGIN=$${JAM_ORIGIN:-host} JAM_PUBLIC_RO=$${JAM_PUBLIC_RO:-1} go run .

# Dev runner: source ../.env, jamd (+ braind if present), trap cleanup
jam-dev:
	./scripts/jam-dev.sh

jam-build:
	cd jamd && go build -o ../bin/jamd .

# Code Jam brain worker — tips → X research cards → jamd
# Requires jamd up. Env from ../.env: JAM_HOST, JAM_ID, JAM_TOKEN, X_BEARER_TOKEN, XAI_API_KEY
braind:
	cd braind && JAM_HOST=$${JAM_HOST:-http://127.0.0.1:7720} JAM_ID=$${JAM_ID:-demo} JAM_TOKEN=$${JAM_TOKEN:-dev} go run .

braind-build:
	cd braind && go build -o ../bin/braind .

publish:
	JAM_HOST=$${JAM_HOST:-http://127.0.0.1:7720} $(PY) jam_publish.py --origin $${JAM_ORIGIN:-member} --once

publish-watch:
	JAM_HOST=$${JAM_HOST:-http://127.0.0.1:7720} $(PY) jam_publish.py --origin $${JAM_ORIGIN:-member} --watch 8

peek:
	$(PY) peek.py --url $${JAM_HOST:-http://127.0.0.1:7720} --watch 2

fixtureserve:
	$(PY) fixtureserve.py

test:
	$(PY) -m pytest tests/ -q
	cd jamd && go test ./...
	cd braind && go test ./...

contract:
	$(PY) -m pytest tests/test_contract.py -q

smoke:
	./scripts/smoke.sh

smoke-jam:
	./scripts/smoke-jam.sh

smoke-bus:
	./scripts/smoke-bus.sh

smoke-sandbox:
	./scripts/smoke-sandbox.sh

# Alice↔Bob SSE + optional sandbox (jamd must be up)
proof-multiuser:
	./scripts/proof-multiuser.sh

# Bring up redis (compose profile) + assert bus=redis when REDIS_URL set
redis-up:
	docker compose --profile redis up -d redis

smoke-brain:
	./scripts/smoke-brain.sh

# Run jam + brain smokes in parallel (distinct origins; jamd must already be up).
# Unset JAM_ORIGIN from .env so both don't collide on the same per-origin rate limit.
smoke-parallel:
	@set -e; \
	env JAM_ORIGIN=smoke-jam-$$$$-$$RANDOM ./scripts/smoke-jam.sh > /tmp/smoke-jam.out 2>&1 & pj=$$!; \
	env JAM_ORIGIN=smoke-brain-$$$$-$$RANDOM SMOKE_BRAIN_RUN=$${SMOKE_BRAIN_RUN:-0} ./scripts/smoke-brain.sh > /tmp/smoke-brain.out 2>&1 & pb=$$!; \
	ej=0; eb=0; wait $$pj || ej=$$?; wait $$pb || eb=$$?; \
	echo "===== smoke-jam ====="; cat /tmp/smoke-jam.out; \
	echo "===== smoke-brain ====="; cat /tmp/smoke-brain.out; \
	echo "===== parallel done jam=$$ej brain=$$eb ====="; \
	test $$ej -eq 0 && test $$eb -eq 0

docker-jam:
	docker compose up --build jam

docker-sandbox:
	docker compose --profile sandbox up sandbox

# Local jam checkpoint (no push). See CHECKPOINT.md
test-jam:
	cd jamd && go test ./...

test-brain:
	cd braind && go test ./...

checkpoint-jam:
	./scripts/checkpoint-jam.sh
