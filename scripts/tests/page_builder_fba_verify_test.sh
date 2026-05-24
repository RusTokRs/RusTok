#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFY_DIR="$REPO_ROOT/scripts/verify"

create_fixture_repo() {
  FIXTURE_ROOT="$(mktemp -d)"
  mkdir -p "$FIXTURE_ROOT/crates/rustok-page-builder" "$FIXTURE_ROOT/crates/rustok-pages" "$FIXTURE_ROOT/scripts/verify"

  cat > "$FIXTURE_ROOT/crates/rustok-page-builder/rustok-module.toml" <<'EOF'
[module]
slug = "page_builder"

[fba.provider]
builder_contract_version = "1.0"
consumer_min_version = "1.0"
EOF

  write_pages_manifest "1.0" "1.0"
}

write_pages_manifest() {
  local contract_version="$1"
  local builder_contract_version="$2"
  cat > "$FIXTURE_ROOT/crates/rustok-pages/rustok-module.toml" <<EOF
[fba.builder_consumer]
contract_version = "${contract_version}"
builder_contract_version = "${builder_contract_version}"

[fba.builder_consumer.degraded_modes]
builder_disabled = "admin_builder_readonly_fallback"
preview_disabled = "preview_capability_hidden_keep_read_paths"
publish_disabled = "typed_feature_disabled_error_keep_read_paths"

[fba.builder_consumer.toggle_profiles]
all_on = [
  "builder.enabled=true",
  "builder.preview.enabled=true",
  "builder.properties.enabled=true",
  "builder.publish.enabled=true",
  "builder.legacy_bridge_readonly=true",
]
publish_off = [
  "builder.enabled=true",
  "builder.preview.enabled=true",
  "builder.properties.enabled=true",
  "builder.publish.enabled=false",
  "builder.legacy_bridge_readonly=true",
]
preview_off = [
  "builder.enabled=true",
  "builder.preview.enabled=false",
  "builder.properties.enabled=true",
  "builder.publish.enabled=false",
  "builder.legacy_bridge_readonly=true",
]
builder_off = [
  "builder.enabled=false",
  "builder.preview.enabled=false",
  "builder.properties.enabled=false",
  "builder.publish.enabled=false",
  "builder.legacy_bridge_readonly=true",
]
EOF
}

copy_verify_scripts() {
  cp "$VERIFY_DIR/verify-page-builder-contract-parity.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-fallback-profiles.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-toggle-profiles-consistency.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-fba-baseline.mjs" "$FIXTURE_ROOT/scripts/verify/"
}

cleanup_fixture_repo() {
  rm -rf "$FIXTURE_ROOT"
  [[ -n "${FAIL_OUTPUT_FILE:-}" ]] && rm -f "$FAIL_OUTPUT_FILE"
  [[ -n "${VERIFY_ALL_OUTPUT_FILE:-}" ]] && rm -f "$VERIFY_ALL_OUTPUT_FILE"
}

test_baseline_passes_on_isolated_fixture() {
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-fallback-profiles.mjs)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs)
}

test_baseline_fails_on_contract_mismatch_fixture() {
  write_pages_manifest "1.0" "2.0"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected contract parity to fail on contract mismatch fixture"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
  grep -q "builder_contract_version mismatch" "$FAIL_OUTPUT_FILE"
}

test_baseline_fails_on_consumer_below_minimum_fixture() {
  cat > "$FIXTURE_ROOT/crates/rustok-page-builder/rustok-module.toml" <<'EOF'
[module]
slug = "page_builder"

[fba.provider]
builder_contract_version = "1.1"
consumer_min_version = "1.1"
EOF

  write_pages_manifest "1.1" "1.0"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected contract parity to fail when consumer version is below provider minimum"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
  grep -q "consumer version below provider minimum" "$FAIL_OUTPUT_FILE"
}

test_baseline_fails_on_invalid_version_format_fixture() {
  write_pages_manifest "1.0" "1.x"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected contract parity to fail on invalid numeric version segment"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
  grep -q "invalid numeric version segment" "$FAIL_OUTPUT_FILE"
}

test_verify_all_alias_runs_page_builder_baseline() {
  VERIFY_ALL_OUTPUT_FILE="$(mktemp)"
  (cd "$REPO_ROOT" && "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >"$VERIFY_ALL_OUTPUT_FILE")
  grep -q "PASS" "$VERIFY_ALL_OUTPUT_FILE"
}

create_fixture_repo
copy_verify_scripts
trap cleanup_fixture_repo EXIT
test_baseline_passes_on_isolated_fixture
test_baseline_fails_on_contract_mismatch_fixture
create_fixture_repo
copy_verify_scripts
test_baseline_fails_on_consumer_below_minimum_fixture
create_fixture_repo
copy_verify_scripts
test_baseline_fails_on_invalid_version_format_fixture
test_verify_all_alias_runs_page_builder_baseline

echo "page_builder_fba_verify_test.sh: PASS (fixture pass/fail mismatch+minimum+invalid-format + repo alias)"
