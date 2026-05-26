#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
cd "$ROOT"

export CI="${CI:-true}"

cargo test --workspace --all-targets
cargo run --bin xtask -- test --test-dir tests
