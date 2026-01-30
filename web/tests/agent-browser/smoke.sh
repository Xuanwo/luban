#!/usr/bin/env bash
set -euo pipefail

if ! command -v agent-browser >/dev/null 2>&1; then
  echo "agent-browser not found; install it to run UI smoke tests" >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1 && ! command -v pnpm.cmd >/dev/null 2>&1; then
  echo "pnpm not found; install pnpm to run UI smoke tests" >&2
  exit 1
fi

PORT="${LUBAN_AGENT_BROWSER_PORT:-3000}"
HOST="127.0.0.1"
BASE_URL="http://${HOST}:${PORT}/"
SESSION="luban-ui-smoke-$$"
AGENT_BROWSER_HEADED_FLAG=""
if [ "${LUBAN_AGENT_BROWSER_HEADED:-0}" = "1" ]; then
  AGENT_BROWSER_HEADED_FLAG="--headed"
fi

if [ ! -d node_modules ]; then
  pnpm install
fi

LOG_FILE="$(mktemp -t luban-agent-browser-ui-smoke-XXXXXX.log)"

NEXT_PUBLIC_LUBAN_MODE=mock pnpm dev -- -p "${PORT}" >"${LOG_FILE}" 2>&1 &
DEV_PID="$!"

cleanup() {
  agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" close >/dev/null 2>&1 || true
  if kill -0 "${DEV_PID}" >/dev/null 2>&1; then
    kill "${DEV_PID}" >/dev/null 2>&1 || true
  fi
  wait "${DEV_PID}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for _ in $(seq 1 120); do
  if curl -fsS "${BASE_URL}" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done

curl -fsS "${BASE_URL}" >/dev/null 2>&1 || {
  echo "web dev server did not become ready at ${BASE_URL}" >&2
  echo "log: ${LOG_FILE}" >&2
  exit 1
}

agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" open "${BASE_URL}"
agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" wait --load networkidle

wait_for_js() {
  local expr="$1"
  for _ in $(seq 1 80); do
    local out
    out="$(agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" eval "Boolean(${expr})" 2>/dev/null || true)"
    if [ "${out}" = "true" ]; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

wait_for_js "document.querySelector('[data-testid=\"nav-sidebar\"]') != null"
wait_for_js "document.querySelector('[data-testid=\"task-list-view\"]') != null"

agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" find testid "new-task-button" click
wait_for_js "document.querySelector('[data-testid=\"new-task-modal\"]') != null"

agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" press Escape

agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" find testid "workspace-switcher-button" click
agent-browser ${AGENT_BROWSER_HEADED_FLAG:+$AGENT_BROWSER_HEADED_FLAG} --session "${SESSION}" find testid "open-settings-button" click
wait_for_js "document.querySelector('[data-testid=\"settings-panel\"]') != null"
