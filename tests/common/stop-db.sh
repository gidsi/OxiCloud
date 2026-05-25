#!/usr/bin/env bash
set -euo pipefail

COMPOSE_FILE="$(dirname "$0")/docker-compose.test.yml"

if ! command -v docker >/dev/null 2>&1; then
  echo "SKIP: docker is not installed; no test postgres to stop" >&2
  exit 77
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "SKIP: docker compose is not installed; no test postgres to stop" >&2
  exit 77
fi

echo "[teardown] Stopping test postgres..."
docker compose -f "$COMPOSE_FILE" down -v
