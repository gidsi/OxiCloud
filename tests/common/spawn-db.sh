#!/usr/bin/env bash
set -euo pipefail

COMPOSE_FILE="$(dirname "$0")/docker-compose.test.yml"

if ! command -v docker >/dev/null 2>&1; then
  echo "SKIP: docker is required to start test postgres" >&2
  exit 77
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "SKIP: docker compose is required to start test postgres" >&2
  exit 77
fi

if ! command -v nc >/dev/null 2>&1; then
  echo "SKIP: nc is required to wait for test postgres" >&2
  exit 77
fi

wait_for_port() {
  local host="$1" port="$2" timeout="${3:-30}"
  local deadline=$(( $(date +%s) + timeout ))
  until nc -z "$host" "$port" 2>/dev/null; do
    [[ $(date +%s) -ge $deadline ]] && echo "Timeout waiting for $host:$port" >&2 && exit 1
    sleep 0.5
  done
}

echo "[setup] Starting test postgres..."
docker compose -f "$COMPOSE_FILE" down -v 2>/dev/null || true
docker compose -f "$COMPOSE_FILE" up -d
echo "[setup] Waiting for postgres on port 5433..."
wait_for_port 127.0.0.1 5433
echo "[setup] Postgres is ready."
