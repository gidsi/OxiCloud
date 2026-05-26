#!/usr/bin/env bash
# Full Hurl API test runner.
# Starts postgres + OxiCloud server, dynamically discovers Hurl tests,
# runs them, and tears everything down.
#
# Usage (from repo root):
#   bash tests/api/run.sh
#
# Prerequisites: docker, cargo or a pre-built OxiCloud binary, hurl ≥ 4.0

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
COMMON="$REPO_ROOT/tests/common"
API_DIR="$REPO_ROOT/tests/api"

# test.env is the single source of truth for connection details and credentials.
# shellcheck source=test.env
source "$API_DIR/test.env"

# Derive server port from base_url (e.g. http://localhost:8087 → 8087)
SERVER_PORT="${base_url##*:}"

# ── Helpers ───────────────────────────────────────────────────────────────────

log()  { echo "[api-test] $*"; }
die()  { echo "[api-test] ERROR: $*" >&2; exit 1; }

wait_for_http() {
  local url="$1" timeout="${2:-60}"
  local deadline=$(( $(date +%s) + timeout ))
  until curl -sf "$url" >/dev/null 2>&1; do
    [[ $(date +%s) -ge $deadline ]] && die "Timeout waiting for $url"
    sleep 1
  done
}

discover_hurl_tests() {
  local setup_file="$API_DIR/setup.hurl"
  local -a discovered_tests=()
  local -a ordered_tests=()

  while IFS= read -r -d '' test_file; do
    discovered_tests+=("$test_file")
  done < <(find "$API_DIR" -type f -name '*.hurl' -print0 | sort -z)

  if [[ ${#discovered_tests[@]} -eq 0 ]]; then
    die "No .hurl API tests discovered under $API_DIR"
  fi

  if [[ -f "$setup_file" ]]; then
    ordered_tests+=("$setup_file")
  fi

  for test_file in "${discovered_tests[@]}"; do
    if [[ "$test_file" != "$setup_file" ]]; then
      ordered_tests+=("$test_file")
    fi
  done

  printf '%s\0' "${ordered_tests[@]}"
}

# ── Teardown (always runs on exit) ────────────────────────────────────────────

SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    log "Stopping OxiCloud server (pid $SERVER_PID)..."
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  bash "$COMMON/stop-db.sh"
}

trap cleanup EXIT

# ── 1. Start postgres ─────────────────────────────────────────────────────────

bash "$COMMON/spawn-db.sh"

# ── 2. Load shared server env + port from .env ───────────────────────────────

set -a
# shellcheck source=../common/server.env
source "$COMMON/server.env"
OXICLOUD_SERVER_PORT=$SERVER_PORT
OXICLOUD_STORAGE_PATH="$REPO_ROOT/tests/api/storage"
set +a

# ensure storage is empty before starting
echo "Wipe $OXICLOUD_STORAGE_PATH to ensure clean startup"
rm -rf "$OXICLOUD_STORAGE_PATH"
mkdir -p "$OXICLOUD_STORAGE_PATH"

# ── 3. Start OxiCloud server ──────────────────────────────────────────────────

BUILD_TARGET="${BUILD_TARGET:-debug}"
OXICLOUD_BIN="$REPO_ROOT/target/$BUILD_TARGET/oxicloud"

if [[ -x "$OXICLOUD_BIN" ]]; then
  log "Starting pre-built OxiCloud server ($BUILD_TARGET) on port $SERVER_PORT..."
  "$OXICLOUD_BIN" &
else
  log "Building and starting OxiCloud server on port $SERVER_PORT..."
  cd "$REPO_ROOT"
  cargo run &
fi
SERVER_PID=$!
log "Waiting for server at $base_url..."
wait_for_http "$base_url/ready" 120
log "Server is ready."

# ── 4. Run Hurl tests ─────────────────────────────────────────────────────────

log "Discovering Hurl tests under $API_DIR..."
mapfile -d '' HURL_TESTS < <(discover_hurl_tests)
for test_file in "${HURL_TESTS[@]}"; do
  log "Discovered Hurl test: $test_file"
done

log "Running ${#HURL_TESTS[@]} Hurl test file(s)..."
hurl --variables-file "$API_DIR/test.env" --file-root "$REPO_ROOT/tests" --test --jobs 1 \
  "${HURL_TESTS[@]}"

bash "$API_DIR/storage_cleanup_check.sh"

log "All tests passed."
