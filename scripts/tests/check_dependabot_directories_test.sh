#!/usr/bin/env bash
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/ci/check-dependabot-directories.py"
TMPDIR_ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "$TMPDIR_ROOT"
}
trap cleanup EXIT

fail() {
  echo "[FAIL] $1" >&2
  exit 1
}

pass() {
  echo "[PASS] $1"
}

test_passes_for_existing_directories() {
  local tmp
  tmp="$(mktemp -d "$TMPDIR_ROOT/ok-test.XXXXXX")"
  mkdir -p "$tmp/.github" "$tmp/apps/admin" "$tmp/apps/server" "$tmp/crates"
  cat > "$tmp/.github/dependabot.yml" <<'YAML'
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "daily"
  - package-ecosystem: "cargo"
    directory: "/apps/admin"
    schedule:
      interval: "daily"
  - package-ecosystem: "cargo"
    directory: "/apps/server"
    schedule:
      interval: "daily"
  - package-ecosystem: "cargo"
    directory: "/crates"
    schedule:
      interval: "daily"
YAML

  local out_log="$tmp/check_dependabot_ok.log"
  python3 "$SCRIPT" --root "$tmp" --config "$tmp/.github/dependabot.yml" >"$out_log"
  rg -q "All Dependabot update directories exist" "$out_log" \
    || fail "expected success message"
  pass "script passes for valid dependabot config in isolated fixture"
}

test_passes_for_recursive_directories() {
  local tmp
  tmp="$(mktemp -d "$TMPDIR_ROOT/recursive-test.XXXXXX")"
  mkdir -p "$tmp/.github" "$tmp/apps/server" "$tmp/crates/rustok-core"
  printf '[workspace]\n' > "$tmp/Cargo.toml"
  printf '[package]\nname = "server"\nversion = "0.1.0"\n' > "$tmp/apps/server/Cargo.toml"
  printf '[package]\nname = "rustok-core"\nversion = "0.1.0"\n' > "$tmp/crates/rustok-core/Cargo.toml"
  cat > "$tmp/.github/dependabot.yml" <<'YAML'
version: 2
updates:
  - package-ecosystem: "cargo"
    directories:
      - "/"
      - "**/*"
    schedule:
      interval: "daily"
YAML

  local out_log="$tmp/check_dependabot_recursive.log"
  python3 "$SCRIPT" --root "$tmp" --config "$tmp/.github/dependabot.yml" >"$out_log"
  rg -q "All Dependabot update directories exist" "$out_log" \
    || fail "expected recursive directory pattern to pass"
  pass "script expands recursive dependabot directory patterns"
}

test_fails_when_cargo_manifest_is_not_covered() {
  local tmp
  tmp="$(mktemp -d "$TMPDIR_ROOT/uncovered-cargo-test.XXXXXX")"
  mkdir -p "$tmp/.github" "$tmp/apps/server" "$tmp/crates/rustok-core"
  printf '[package]\nname = "server"\nversion = "0.1.0"\n' > "$tmp/apps/server/Cargo.toml"
  printf '[package]\nname = "rustok-core"\nversion = "0.1.0"\n' > "$tmp/crates/rustok-core/Cargo.toml"
  cat > "$tmp/.github/dependabot.yml" <<'YAML'
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/apps/server"
    schedule:
      interval: "daily"
YAML

  set +e
  python3 "$SCRIPT" --root "$tmp" --config "$tmp/.github/dependabot.yml" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected exit code 1 for uncovered Cargo manifest"
  rg -q "Dependabot Cargo configuration does not cover Cargo manifests" "$tmp/out.log" \
    || fail "expected Cargo coverage failure heading"
  rg -q "crates/rustok-core" "$tmp/out.log" \
    || fail "expected uncovered Cargo manifest directory"
  pass "script fails when a Cargo manifest is outside Dependabot coverage"
}

test_fails_for_missing_directory() {
  local tmp
  tmp="$(mktemp -d "$TMPDIR_ROOT/missing-dir-test.XXXXXX")"
  mkdir -p "$tmp/.github" "$tmp/apps/admin"
  cat > "$tmp/.github/dependabot.yml" <<'YAML'
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/apps/admin"
    schedule:
      interval: "daily"
  - package-ecosystem: "cargo"
    directory: "/apps/does-not-exist"
    schedule:
      interval: "daily"
YAML

  set +e
  python3 "$SCRIPT" --root "$tmp" --config "$tmp/.github/dependabot.yml" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected exit code 1 for missing directory"
  rg -q "Dependabot directories do not exist" "$tmp/out.log" || fail "expected failure heading"
  rg -q "apps/does-not-exist" "$tmp/out.log" || fail "expected missing directory in output"
  pass "script fails when dependabot contains missing directory"
}

test_fails_when_config_is_missing() {
  local tmp
  tmp="$(mktemp -d "$TMPDIR_ROOT/missing-config-test.XXXXXX")"
  mkdir -p "$tmp"

  set +e
  python3 "$SCRIPT" --root "$tmp" --config "$tmp/.github/dependabot.yml" >"$tmp/out.log" 2>&1
  local code=$?
  set -e

  [[ $code -eq 1 ]] || fail "expected exit code 1 for missing dependabot config"
  rg -q "Dependabot config file not found" "$tmp/out.log" || fail "expected missing config message"
  pass "script fails with clear message when dependabot config is missing"
}

test_passes_for_existing_directories
test_passes_for_recursive_directories
test_fails_when_cargo_manifest_is_not_covered
test_fails_for_missing_directory
test_fails_when_config_is_missing

echo "check_dependabot_directories tests passed"
