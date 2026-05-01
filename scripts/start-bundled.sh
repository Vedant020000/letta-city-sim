#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${SIM_API_KEY:?SIM_API_KEY is required}"

APP_PORT="${PORT:-3000}"
API_PORT="${WORLD_API_PORT:-3001}"
DB_WAIT_ATTEMPTS="${DB_WAIT_ATTEMPTS:-60}"
DB_WAIT_SECONDS="${DB_WAIT_SECONDS:-2}"

wait_for_db() {
  local attempt=1
  while [[ "${attempt}" -le "${DB_WAIT_ATTEMPTS}" ]]; do
    if psql "${DATABASE_URL}" -X -v ON_ERROR_STOP=1 -c "SELECT 1;" >/dev/null 2>&1; then
      return 0
    fi

    echo "Waiting for database (${attempt}/${DB_WAIT_ATTEMPTS})..."
    sleep "${DB_WAIT_SECONDS}"
    attempt=$((attempt + 1))
  done

  echo "Database did not become ready in time." >&2
  exit 1
}

cleanup() {
  if [[ -n "${FRONTEND_PID:-}" ]]; then
    kill "${FRONTEND_PID}" 2>/dev/null || true
  fi
  if [[ -n "${API_PID:-}" ]]; then
    kill "${API_PID}" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

echo "Checking database connectivity..."
wait_for_db

echo "Starting bundled world-api on port ${API_PORT}..."
PORT="${API_PORT}" /app/bin/world-api &
API_PID=$!

echo "Starting bundled frontend on port ${APP_PORT}..."
cd /app/frontend
PORT="${APP_PORT}" \
INTERNAL_API_ORIGIN="http://127.0.0.1:${API_PORT}" \
INTERNAL_WS_ORIGIN="ws://127.0.0.1:${API_PORT}" \
node ./server.mjs &
FRONTEND_PID=$!

wait -n "${API_PID}" "${FRONTEND_PID}"
STATUS=$?

cleanup
wait || true

exit "${STATUS}"
