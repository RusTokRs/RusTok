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
builder_contract_version = "1.0"
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/rustok-module.toml" <<'EOF'
[fba.builder_consumer]
contract_version = "1.0"
builder_contract_version = "1.0"

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
]
publish_off = [
  "builder.enabled=true",
  "builder.preview.enabled=true",
  "builder.properties.enabled=true",
  "builder.publish.enabled=false",
]
preview_off = [
  "builder.enabled=true",
  "builder.preview.enabled=false",
  "builder.properties.enabled=true",
  "builder.publish.enabled=true",
]
builder_off = [
  "builder.enabled=false",
  "builder.preview.enabled=false",
  "builder.properties.enabled=false",
  "builder.publish.enabled=false",
]
EOF

  cp "$VERIFY_DIR/verify-page-builder-contract-parity.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-fallback-profiles.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-toggle-profiles-consistency.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-fba-baseline.mjs" "$FIXTURE_ROOT/scripts/verify/"
}

cleanup_fixture_repo() {
  rm -rf "$FIXTURE_ROOT"
}

test_baseline_passes_on_isolated_fixture() {
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-fba-baseline.mjs)
}

test_verify_all_alias_runs_page_builder_baseline() {
  (cd "$REPO_ROOT" && "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >/tmp/page_builder_verify_all.out)
  grep -q "PASS" /tmp/page_builder_verify_all.out
}

create_fixture_repo
trap cleanup_fixture_repo EXIT
test_baseline_passes_on_isolated_fixture
test_verify_all_alias_runs_page_builder_baseline

echo "page_builder_fba_verify_test.sh: PASS (fixture + repo alias)"
