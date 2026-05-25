#!/usr/bin/env bash
set -uo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

FAILURES=0
STRICT_E2E="${STRICT_E2E:-0}"

skip() {
  echo "↷ SKIP: $*" >&2
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

run_cmd() {
  local desc="$1"
  shift
  echo
  echo "==> ${desc}"
  if "$@"; then
    echo "✓ ${desc}"
  else
    local status=$?
    if [ "$status" -eq 77 ]; then
      echo "↷ ${desc} skipped"
      return 0
    fi
    echo "✗ ${desc} failed with exit code ${status}" >&2
    FAILURES=1
    return "$status"
  fi
}

run_in_dir() {
  local desc="$1"
  local dir="$2"
  shift 2
  run_cmd "$desc" bash -c 'cd "$1" && shift && "$@"' bash "$dir" "$@"
}

if [ -f "$ROOT/Cargo.toml" ]; then
  if have_cmd cargo; then
    run_cmd "cargo test --workspace --all-features" cargo test --workspace --all-features
  else
    echo "✗ Cargo.toml found but cargo is not installed" >&2
    FAILURES=1
  fi
fi

mapfile -d '' PACKAGE_JSON_FILES < <(find "$ROOT" \
  -path '*/node_modules' -prune -o \
  -path '*/target' -prune -o \
  -name package.json -type f -print0 | sort -z)

for package_json in "${PACKAGE_JSON_FILES[@]}"; do
  pkg_dir="$(dirname "$package_json")"
  rel_dir="${pkg_dir#$ROOT/}"

  if ! have_cmd npm; then
    if [ "$STRICT_E2E" = "1" ]; then
      echo "✗ package.json found in ${rel_dir}, but npm is not installed" >&2
      FAILURES=1
    else
      skip "package.json found in ${rel_dir}, but npm is not installed"
    fi
    continue
  fi

  if [ -f "$pkg_dir/package-lock.json" ]; then
    run_in_dir "npm ci in ${rel_dir}" "$pkg_dir" npm ci
  else
    run_in_dir "npm install in ${rel_dir}" "$pkg_dir" npm install
  fi

  has_test_script=""
  if have_cmd node; then
    has_test_script="$(cd "$pkg_dir" && node -e 'const p=require("./package.json"); process.stdout.write(p.scripts && p.scripts.test ? "yes" : "");' 2>/dev/null || true)"
  fi

  has_playwright_config=""
  if [ -f "$pkg_dir/playwright.config.ts" ] || [ -f "$pkg_dir/playwright.config.js" ] || [ -f "$pkg_dir/playwright.config.mjs" ] || [ -f "$pkg_dir/playwright.config.cjs" ]; then
    has_playwright_config="yes"
  fi

  if [ "$has_playwright_config" = "yes" ]; then
    run_in_dir "playwright browser install in ${rel_dir}" "$pkg_dir" npx playwright install
  fi

  if [ "$has_test_script" = "yes" ]; then
    run_in_dir "npm test in ${rel_dir}" "$pkg_dir" npm test
  elif [ "$has_playwright_config" = "yes" ]; then
    run_in_dir "playwright tests in ${rel_dir}" "$pkg_dir" npx playwright test
  fi
done

mapfile -d '' HURL_FILES_ALL < <(find "$ROOT/tests" -type f -name '*.hurl' -print0 2>/dev/null | sort -z)

if [ "${#HURL_FILES_ALL[@]}" -gt 0 ]; then
  if ! have_cmd hurl; then
    skip ".hurl tests found but hurl is not installed"
  else
    skip ".hurl tests are server-dependent and are run through tests/api/run.sh"
  fi
fi

mapfile -d '' SHELL_FILES_ALL < <(find "$ROOT/tests" -type f -name '*.sh' -print0 2>/dev/null | sort -z)

for shell_file in "${SHELL_FILES_ALL[@]}"; do
  rel_shell="${shell_file#$ROOT/}"
  shell_base="$(basename "$shell_file")"
  shell_dir="$(dirname "$shell_file")"

  case "$rel_shell" in
    tests/common/*|*/common.sh|*/start-server.sh)
      continue
      ;;
    tests/api/run.sh|tests/webdav/run.sh)
      run_in_dir "shell test ${rel_shell}" "$shell_dir" bash "./$shell_base"
      ;;
    tests/api/*.sh|tests/webdav/test_*.sh|tests/e2e/*_test.sh)
      skip "${rel_shell} is server-dependent and is run only through its suite entrypoint"
      ;;
    *)
      run_in_dir "shell test ${rel_shell}" "$shell_dir" bash "./$shell_base"
      ;;
  esac
done

if [ "$FAILURES" -ne 0 ]; then
  echo
  echo "One or more tests failed." >&2
  exit 1
fi

echo
echo "All runnable tests passed."
exit 0
