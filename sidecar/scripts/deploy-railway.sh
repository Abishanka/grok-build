#!/usr/bin/env bash
# Deploy jamd (public edge) + braind (worker) to Railway.
# Usage (from sidecar/):
#   ./scripts/deploy-railway.sh
#   ./scripts/deploy-railway.sh jamd|braind|both
#
# Project defaults to disciplined-friendship (Code Jam). Override with:
#   RAILWAY_PROJECT_ID=... RAILWAY_ENVIRONMENT=production
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PROJECT="${RAILWAY_PROJECT_ID:-a23cd1a7-4043-4c26-b3c5-1796aaace149}"
ENV_NAME="${RAILWAY_ENVIRONMENT:-production}"
TARGET="${1:-both}"
export RAILWAY_CALLER="${RAILWAY_CALLER:-skill:use-railway@1.3.7}"
export RAILWAY_AGENT_SESSION="${RAILWAY_AGENT_SESSION:-deploy-$(date +%s)-$$}"

info() { echo "deploy: $*" >&2; }
fail() { echo "FAIL: $*" >&2; exit 1; }

command -v railway >/dev/null || fail "railway CLI missing"
command -v jq >/dev/null || fail "jq required"

ensure_braind_service() {
  local list
  list=$(railway service list --project "$PROJECT" --environment "$ENV_NAME" --json 2>/dev/null || echo '[]')
  if echo "$list" | jq -e '.[] | select(.name=="braind")' >/dev/null 2>&1; then
    info "braind service exists"
    return
  fi
  info "creating braind service"
  railway add --service braind --json \
    --project "$PROJECT" 2>/dev/null \
    || railway add --service braind --json
}

configure_braind() {
  info "configuring braind build/deploy + env"
  # Upload root is sidecar/braind/ — Dockerfile lives there.
  # Critical: do NOT inherit jamd's railway.toml healthcheck.
  railway environment edit \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service-config braind \
    build.builder DOCKERFILE 2>/dev/null || true
  railway environment edit \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service-config braind \
    build.dockerfilePath "Dockerfile" 2>/dev/null || true
  railway environment edit \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service-config braind \
    deploy.startCommand "/app/braind" 2>/dev/null || true
  railway environment edit \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service-config braind \
    deploy.restartPolicyType ON_FAILURE 2>/dev/null || true
  # Clear any healthcheck inherited from jamd deploy (worker has no HTTP).
  railway environment edit \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service-config braind \
    deploy.healthcheckPath "" 2>/dev/null || true

  # Private network → jamd. Public edge stays jamd only.
  railway variable set \
    --project "$PROJECT" \
    --environment "$ENV_NAME" \
    --service braind \
    --skip-deploys \
    JAM_HOST='http://${{jamd.RAILWAY_PRIVATE_DOMAIN}}:${{jamd.PORT}}' \
    JAM_ID='${{jamd.JAM_ID}}' \
    JAM_TOKEN='${{jamd.JAM_TOKEN}}' \
    X_BEARER_TOKEN='${{jamd.X_BEARER_TOKEN}}' \
    XAI_API_KEY='${{jamd.XAI_API_KEY}}' \
    XAI_BASE_URL='https://api.x.ai/v1' \
    BRAIN_POLL_INTERVAL=5s \
    BRAIN_DEBOUNCE=15s \
    BRAIN_MAX_CARDS=5 \
    BRAIN_ENABLE_XAI_SEARCH=0 \
    2>&1 | tail -5 || true
}

wait_success() {
  local svc="$1" i status
  info "waiting for $svc deployment SUCCESS"
  for i in $(seq 1 60); do
    status=$(railway deployment list \
      --project "$PROJECT" \
      --environment "$ENV_NAME" \
      --service "$svc" \
      --json 2>/dev/null | jq -r '.[0].status // empty')
    info "  $svc status=$status ($i)"
    case "$status" in
      SUCCESS) return 0 ;;
      FAILED|CRASHED|REMOVED) fail "$svc deployment $status" ;;
      NEEDS_APPROVAL) fail "$svc needs approval" ;;
    esac
    sleep 8
  done
  fail "$svc deploy timeout (last=$status)"
}

deploy_svc() {
  local svc="$1" msg="$2"
  info "railway up → $svc"
  if [[ "$svc" == "braind" ]]; then
    # braind/ as root so its railway.toml wins (no jamd healthcheck).
    (cd "$ROOT/braind" && railway up . \
      --path-as-root \
      --project "$PROJECT" \
      --environment "$ENV_NAME" \
      --service braind \
      --detach \
      -m "$msg")
  else
    # jamd needs jamd/ + web/ from sidecar/
    railway up . \
      --path-as-root \
      --project "$PROJECT" \
      --environment "$ENV_NAME" \
      --service "$svc" \
      --detach \
      -m "$msg"
  fi
  wait_success "$svc"
}

case "$TARGET" in
  jamd)
    deploy_svc jamd "jamd: public edge redeploy"
    ;;
  braind)
    ensure_braind_service
    configure_braind
    deploy_svc braind "braind: phase4 worker → jamd private"
    ;;
  both)
    ensure_braind_service
    configure_braind
    deploy_svc jamd "jamd: public edge redeploy"
    deploy_svc braind "braind: phase4 worker → jamd private"
    ;;
  *)
    fail "usage: $0 [jamd|braind|both]"
    ;;
esac

info "done. public edge: https://jamd-production.up.railway.app/health"
info "lurker: https://jamd-production.up.railway.app/jam.html?jam=demo"
info "smoke:  JAM_HOST=https://jamd-production.up.railway.app ./scripts/smoke-prod.sh"
