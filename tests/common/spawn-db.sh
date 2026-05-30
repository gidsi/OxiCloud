#!/usr/bin/env bash
set -euo pipefail

COMPOSE_FILE="$(dirname "$0")/docker-compose.test.yml"

wait_for_db() {
  local timeout="${1:-30}"
  local deadline=$(( $(date +%s) + timeout ))
  until docker compose -f "$COMPOSE_FILE" exec -T postgres-test pg_isready -U oxicloud_test 2>/dev/null; do
    [[ $(date +%s) -ge $deadline ]] && echo "Timeout waiting for DB healthcheck" >&2 && exit 1
    sleep 0.5
  done
}

echo "[setup] Starting test postgres..."
docker compose -f "$COMPOSE_FILE" down -v 2>/dev/null || true
docker compose -f "$COMPOSE_FILE" up -d
echo "[setup] Waiting for postgres on port 5433..."
wait_for_db 30
echo "[setup] Postgres is ready."
