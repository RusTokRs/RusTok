#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFY_DIR="$REPO_ROOT/scripts/verify"

create_fixture_repo() {
  FIXTURE_ROOT="$(mktemp -d)"
  mkdir -p "$FIXTURE_ROOT/crates/rustok-page-builder" "$FIXTURE_ROOT/crates/rustok-pages" "$FIXTURE_ROOT/crates/rustok-forum" "$FIXTURE_ROOT/scripts/verify" "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components" "$FIXTURE_ROOT/apps/next-admin/src/features/blog/api" "$FIXTURE_ROOT/crates/rustok-pages/admin/locales" "$FIXTURE_ROOT/crates/rustok-pages/admin/src"

  cat > "$FIXTURE_ROOT/crates/rustok-page-builder/rustok-module.toml" <<'EOF'
[module]
slug = "page_builder"
builder_contract_version = "1.0"
EOF


  mkdir -p "$FIXTURE_ROOT/crates/rustok-pages/docs"
  cat > "$FIXTURE_ROOT/crates/rustok-pages/docs/implementation-plan.md" <<'EOF'
# План реализации `rustok-pages`

## Execution checkpoint

- Current phase: fixture
- Notes: FBA page-builder readiness fixture.
EOF


  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/post-form.tsx" <<'EOF'
export const PostForm = () => null;
EOF

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/page-builder.tsx" <<'EOF'
export const PageBuilder = () => null;
EOF

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/api/posts.ts" <<'EOF'
export const postsApi = {};
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/src/lib.rs" <<'EOF'
pub fn placeholder() {}
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/en.json" <<'EOF'
{}
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/ru.json" <<'EOF'
{}
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

  cp "$VERIFY_DIR/verify-page-builder-contract-parity.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-consumer-readiness.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-fallback-profiles.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-toggle-profiles-consistency.mjs" "$FIXTURE_ROOT/scripts/verify/"
  cp "$VERIFY_DIR/verify-page-builder-terminology.mjs" "$FIXTURE_ROOT/scripts/verify/"

  mkdir -p "$FIXTURE_ROOT/crates/rustok-forum/docs"
  cat > "$FIXTURE_ROOT/crates/rustok-forum/docs/implementation-plan.md" <<'EOF'
# Forum implementation

## Execution checkpoint

- Current phase: fixture
- Notes: builder consumer readiness fixture.
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-forum/rustok-module.toml" <<'EOF'
[module]
slug = "forum"

[dependencies]
page_builder = "*"

[fba.builder_consumer]
contract_version = "1.0"
builder_contract_version = "1.0"
EOF

  cp "$VERIFY_DIR/verify-page-builder-fba-baseline.mjs" "$FIXTURE_ROOT/scripts/verify/"
}

cleanup_fixture_repo() {
  rm -rf "$FIXTURE_ROOT"
  [[ -n "${FAIL_OUTPUT_FILE:-}" ]] && rm -f "$FAIL_OUTPUT_FILE"
  [[ -n "${VERIFY_ALL_OUTPUT_FILE:-}" ]] && rm -f "$VERIFY_ALL_OUTPUT_FILE"
  [[ -n "${VERIFY_ALL_FORUM_OUTPUT_FILE:-}" ]] && rm -f "$VERIFY_ALL_FORUM_OUTPUT_FILE"
}

test_baseline_passes_on_isolated_fixture() {
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-consumer-readiness.mjs pages)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-fallback-profiles.mjs)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-consumer-readiness.mjs forum)
  (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-fba-baseline.mjs pages)
}

test_baseline_fails_on_contract_mismatch_fixture() {

  mkdir -p "$FIXTURE_ROOT/crates/rustok-pages/docs"
  cat > "$FIXTURE_ROOT/crates/rustok-pages/docs/implementation-plan.md" <<'EOF'
# План реализации `rustok-pages`

## Execution checkpoint

- Current phase: fixture
- Notes: FBA page-builder readiness fixture.
EOF


  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/post-form.tsx" <<'EOF'
export const PostForm = () => null;
EOF

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/page-builder.tsx" <<'EOF'
export const PageBuilder = () => null;
EOF

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/api/posts.ts" <<'EOF'
export const postsApi = {};
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/src/lib.rs" <<'EOF'
pub fn placeholder() {}
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/en.json" <<'EOF'
{}
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/ru.json" <<'EOF'
{}
EOF

  cat > "$FIXTURE_ROOT/crates/rustok-pages/rustok-module.toml" <<'EOF'
[fba.builder_consumer]
contract_version = "1.0"
builder_contract_version = "2.0"

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

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node scripts/verify/verify-page-builder-contract-parity.mjs >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected baseline to fail on contract mismatch fixture"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
}

test_verify_all_alias_runs_page_builder_baseline() {
  VERIFY_ALL_OUTPUT_FILE="$(mktemp)"
  (cd "$REPO_ROOT" && "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >"$VERIFY_ALL_OUTPUT_FILE")
  grep -q "PASS" "$VERIFY_ALL_OUTPUT_FILE"
}

test_verify_all_alias_runs_page_builder_baseline_for_forum_module() {
  VERIFY_ALL_FORUM_OUTPUT_FILE="$(mktemp)"
  (cd "$REPO_ROOT" && PBC_MODULE=forum "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >"$VERIFY_ALL_FORUM_OUTPUT_FILE")
  grep -q "PASS" "$VERIFY_ALL_FORUM_OUTPUT_FILE"
}

create_fixture_repo
trap cleanup_fixture_repo EXIT
test_baseline_passes_on_isolated_fixture
test_baseline_fails_on_contract_mismatch_fixture
test_verify_all_alias_runs_page_builder_baseline
test_verify_all_alias_runs_page_builder_baseline_for_forum_module

echo "page_builder_fba_verify_test.sh: PASS (fixture pass/fail + repo alias + forum module alias)"
