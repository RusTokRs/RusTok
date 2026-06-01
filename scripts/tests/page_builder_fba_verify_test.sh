#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFY_DIR="$REPO_ROOT/scripts/verify"
PB_VERIFY_DIR="$REPO_ROOT/crates/rustok-page-builder/scripts/verify"

write_terminology_fixture_files() {
  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/post-form.tsx" <<'EOT'
export const PostForm = () => null;
EOT

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components/page-builder.tsx" <<'EOT'
export const PageBuilder = () => null;
EOT

  cat > "$FIXTURE_ROOT/apps/next-admin/src/features/blog/api/posts.ts" <<'EOT'
export const postsApi = {};
EOT

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/src/lib.rs" <<'EOT'
pub fn placeholder() {}
EOT

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/en.json" <<'EOT'
{}
EOT

  cat > "$FIXTURE_ROOT/crates/rustok-pages/admin/locales/ru.json" <<'EOT'
{}
EOT
}

write_pages_impl_plan_fixture() {
  cat > "$FIXTURE_ROOT/crates/rustok-pages/docs/implementation-plan.md" <<'EOT'
# План реализации `rustok-pages`

## Execution checkpoint

- Current phase: fixture
- Notes: FBA page-builder readiness fixture.
EOT
}

write_pages_manifest_fixture() {
  local builder_contract_version="$1"
  local consumer_min_version="${2:-1.0}"
  cat > "$FIXTURE_ROOT/crates/rustok-pages/rustok-module.toml" <<EOT
[module]
slug = "pages"

[dependencies.page_builder]
module = "page-builder"
contract = "grapesjs_v1"
contract_version = "1.0"

[fba.builder_consumer]
provider_module = "page-builder"
contract = "grapesjs_v1"
contract_version = "1.0"
builder_contract_version = "${builder_contract_version}"
consumer_min_version = "${consumer_min_version}"
capabilities = ["preview", "tree", "properties", "publish"]

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
EOT
}

write_registry_fixture() {
  cat > "$FIXTURE_ROOT/crates/rustok-page-builder/contracts/page-builder-fba-registry.json" <<'EOT'
{
  "schema_version": 1,
  "provider": {
    "module_slug": "page_builder",
    "manifest_module": "page-builder",
    "contract": "grapesjs_v1",
    "builder_contract_version": "1.0",
    "consumer_min_version": "1.0",
    "capabilities": ["preview", "tree", "properties", "publish"]
  },
  "consumers": [
    {
      "module_slug": "pages",
      "crate": "rustok-pages",
      "provider_module": "page-builder",
      "contract": "grapesjs_v1",
      "contract_version": "1.0",
      "builder_contract_version": "1.0",
      "consumer_min_version": "1.0",
      "capabilities": ["preview", "tree", "properties", "publish"],
      "rollout_state": "wave0_baseline"
    },
    {
      "module_slug": "forum",
      "crate": "rustok-forum",
      "provider_module": "page-builder",
      "contract": "grapesjs_v1",
      "contract_version": "1.0",
      "builder_contract_version": "1.0",
      "consumer_min_version": "1.0",
      "capabilities": ["preview", "tree", "properties", "publish"],
      "rollout_state": "deferred"
    }
  ],
  "fallback_profiles": ["all_on", "publish_off", "preview_off", "builder_off"]
}
EOT
}

create_fixture_repo() {
  FIXTURE_ROOT="$(mktemp -d)"
  mkdir -p \
    "$FIXTURE_ROOT/crates/rustok-page-builder/contracts" \
    "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify" \
    "$FIXTURE_ROOT/crates/rustok-pages/docs" \
    "$FIXTURE_ROOT/crates/rustok-pages/admin/locales" \
    "$FIXTURE_ROOT/crates/rustok-pages/admin/src" \
    "$FIXTURE_ROOT/crates/rustok-forum/docs" \
    "$FIXTURE_ROOT/apps/next-admin/src/features/blog/components" \
    "$FIXTURE_ROOT/apps/next-admin/src/features/blog/api"

  cat > "$FIXTURE_ROOT/crates/rustok-page-builder/rustok-module.toml" <<'EOT'
[module]
slug = "page_builder"

[fba.provider]
contract = "grapesjs_v1"
builder_contract_version = "1.0"
consumer_min_version = "1.0"
capabilities = ["preview", "tree", "properties", "publish"]
EOT

  write_pages_impl_plan_fixture
  write_terminology_fixture_files
  write_pages_manifest_fixture "1.0"
  write_registry_fixture

  cat > "$FIXTURE_ROOT/crates/rustok-forum/docs/implementation-plan.md" <<'EOT'
# Forum implementation

## Execution checkpoint

- Current phase: fixture
- Notes: builder consumer readiness fixture.
EOT

  cat > "$FIXTURE_ROOT/crates/rustok-forum/rustok-module.toml" <<'EOT'
[module]
slug = "forum"

[dependencies]
page_builder = "*"

[fba.builder_consumer]
provider_module = "page-builder"
contract = "grapesjs_v1"
contract_version = "1.0"
builder_contract_version = "1.0"
consumer_min_version = "1.0"
capabilities = ["preview", "tree", "properties", "publish"]

[fba.builder_consumer.degraded_modes]
builder_disabled = "forum_widgets_readonly_keep_forum_routes"
preview_disabled = "forum_widget_preview_hidden_keep_forum_routes"
publish_disabled = "forum_widget_publish_feature_disabled_keep_forum_routes"

[fba.builder_consumer.toggle_profiles]
all_on = ["builder.enabled=true", "builder.preview.enabled=true", "builder.properties.enabled=true", "builder.publish.enabled=true", "builder.legacy_bridge_readonly=true"]
publish_off = ["builder.enabled=true", "builder.preview.enabled=true", "builder.properties.enabled=true", "builder.publish.enabled=false", "builder.legacy_bridge_readonly=true"]
preview_off = ["builder.enabled=true", "builder.preview.enabled=false", "builder.properties.enabled=true", "builder.publish.enabled=false", "builder.legacy_bridge_readonly=true"]
builder_off = ["builder.enabled=false", "builder.preview.enabled=false", "builder.properties.enabled=false", "builder.publish.enabled=false", "builder.legacy_bridge_readonly=true"]
EOT
}

copy_verify_scripts() {
  cp "$PB_VERIFY_DIR/verify-page-builder-contract-parity.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-contract-registry.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-consumer-readiness.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-fallback-profiles.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-toggle-profiles-consistency.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-terminology.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"
  cp "$PB_VERIFY_DIR/verify-page-builder-fba-baseline.mjs" "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/"

  for script in \
    verify-page-builder-fallback-matrix-docs.mjs \
    verify-page-builder-runtime-fallback-gate.mjs \
    verify-page-builder-pages-fallback-gate.mjs; do
    cat > "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/$script" <<EOT
#!/usr/bin/env node
console.log("[$script] PASS fixture stub");
EOT
    chmod +x "$FIXTURE_ROOT/crates/rustok-page-builder/scripts/verify/$script"
  done
}

cleanup_fixture_repo() {
  rm -rf "${FIXTURE_ROOT:-}"
  [[ -n "${FAIL_OUTPUT_FILE:-}" ]] && rm -f "$FAIL_OUTPUT_FILE"
  [[ -n "${VERIFY_ALL_OUTPUT_FILE:-}" ]] && rm -f "$VERIFY_ALL_OUTPUT_FILE"
  [[ -n "${VERIFY_ALL_FORUM_OUTPUT_FILE:-}" ]] && rm -f "$VERIFY_ALL_FORUM_OUTPUT_FILE"
}

test_baseline_passes_on_isolated_fixture() {
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs pages)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs pages)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs pages)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs pages)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs pages)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs forum)
  (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages)
}

test_baseline_fails_on_contract_mismatch_fixture() {
  write_pages_impl_plan_fixture
  write_terminology_fixture_files
  write_pages_manifest_fixture "2.0"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs pages >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected baseline to fail on contract mismatch fixture"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
  grep -q "builder_contract_version mismatch" "$FAIL_OUTPUT_FILE"
}

test_baseline_fails_on_consumer_below_minimum_fixture() {
  write_pages_manifest_fixture "0.9" "1.0"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs pages >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected baseline to fail on below-minimum fixture"
    cat "$FAIL_OUTPUT_FILE"
    exit 1
  fi
  grep -q "below provider consumer_min_version" "$FAIL_OUTPUT_FILE"
}

test_baseline_fails_on_invalid_version_format_fixture() {
  write_pages_manifest_fixture "1.x" "1.0"

  FAIL_OUTPUT_FILE="$(mktemp)"
  if (cd "$FIXTURE_ROOT" && node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs pages >"$FAIL_OUTPUT_FILE" 2>&1); then
    echo "expected baseline to fail on invalid version fixture"
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

test_verify_all_alias_runs_page_builder_baseline_for_forum_module() {
  VERIFY_ALL_FORUM_OUTPUT_FILE="$(mktemp)"
  (cd "$REPO_ROOT" && PBC_MODULE=forum "$VERIFY_DIR/verify-all.sh" page-builder-fba-baseline >"$VERIFY_ALL_FORUM_OUTPUT_FILE")
  grep -q "PASS" "$VERIFY_ALL_FORUM_OUTPUT_FILE"
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
test_verify_all_alias_runs_page_builder_baseline_for_forum_module

echo "page_builder_fba_verify_test.sh: PASS (fixture pass/fail + repo alias + forum module alias)"
