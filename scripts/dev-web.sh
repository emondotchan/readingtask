#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_URL="${WEB_URL:-http://127.0.0.1:1420}"
BACKEND_BIND="${READING_TASK_BIND:-0.0.0.0:10086}"
BACKEND_URL="${BACKEND_URL:-http://127.0.0.1:10086/api/health}"

open_browser() {
  local url="$1"

  if command -v open >/dev/null 2>&1; then
    open "$url"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$url"
  elif command -v cmd.exe >/dev/null 2>&1; then
    cmd.exe /c start "" "$url"
  else
    printf '%s\n' "$url"
  fi
}

cleanup() {
  if [[ -n "${FRONTEND_PID:-}" ]] && kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
    kill "$FRONTEND_PID" >/dev/null 2>&1 || true
  fi

  if [[ -n "${BACKEND_PID:-}" ]] && kill -0 "$BACKEND_PID" >/dev/null 2>&1; then
    kill "$BACKEND_PID" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

(
  cd "$ROOT_DIR"
  READING_TASK_BIND="$BACKEND_BIND" cargo run
) &
BACKEND_PID=$!

(
  cd "$ROOT_DIR/web"
  npm run dev
) &
FRONTEND_PID=$!

until curl -fsS "$WEB_URL" >/dev/null 2>&1 && curl -fsS "$BACKEND_URL" >/dev/null 2>&1; do
  if ! kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
    wait "$FRONTEND_PID"
  fi
  if ! kill -0 "$BACKEND_PID" >/dev/null 2>&1; then
    wait "$BACKEND_PID"
  fi
  sleep 1
done

open_browser "$WEB_URL"

wait "$FRONTEND_PID" "$BACKEND_PID"
