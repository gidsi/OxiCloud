#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace --all-targets
cargo run -p xtask -- test --tests-dir tests --timeout-secs "${TEST_SCRIPT_TIMEOUT:-30}"
