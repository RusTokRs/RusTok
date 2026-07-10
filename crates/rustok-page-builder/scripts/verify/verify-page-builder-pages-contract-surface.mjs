#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const checks = [
  {
    file: "crates/rustok-pages/tests/integration.rs",
    label: "page CRUD and body sanitization contract tests",
    tokens: [
      "test_page_lifecycle",
      "test_create_page_rt_json_v1_sanitizes_payload",
      "test_update_page_rt_json_v1_sanitizes_payload",
    ],
  },
  {
    file: "crates/rustok-pages/tests/page_builder_roundtrip.rs",
    label: "visual builder and legacy block bridge contract tests",
    tokens: [
      "grapesjs_body_round_trips_on_create_and_get",
      "grapesjs_body_round_trips_on_update",
      "legacy_block_driven_page_round_trips_without_body",
      "grapesjs_body_update_preserves_legacy_blocks",
    ],
  },
  {
    file: "crates/rustok-pages/tests/page_service_kind_guard.rs",
    label: "builder fallback and page/block kind guard contract tests",
    tokens: [
      "publish_returns_page_not_found_for_block_id_and_keeps_page_status",
      "delete_returns_page_not_found_for_block_id_and_keeps_page_record",
      "pages_builder_fallback_all_on_allows_publish_and_keeps_read_list_paths",
      "pages_builder_fallback_publish_off_blocks_grapesjs_publish_but_keeps_read_list_paths",
      "pages_builder_fallback_preview_off_blocks_preview_publish_but_keeps_read_list_paths",
      "pages_builder_fallback_builder_off_keeps_read_and_list_paths",
      "preview_capability_returns_feature_disabled_when_preview_toggle_is_false",
      "properties_capability_returns_feature_disabled_when_properties_toggle_is_false",
    ],
  },
  {
    file: "crates/rustok-pages/tests/menu_service.rs",
    label: "menu service contract tests",
    tokens: ["menu_round_trip_uses_module_owned_storage"],
  },
  {
    file: "crates/rustok-pages/tests/page_locale_fallback.rs",
    label: "page locale fallback contract tests",
    tokens: [
      "get_by_slug_falls_back_to_platform_locale",
      "get_by_slug_respects_explicit_fallback_locale",
      "get_with_locale_fallback_normalizes_requested_and_fallback_locale",
    ],
  },
  {
    file: "crates/rustok-pages/tests/rbac.rs",
    label: "RBAC and channel visibility contract tests",
    tokens: [
      "manager_cannot_publish_via_create_or_update",
      "customer_cannot_read_drafts_and_list_only_returns_published_pages",
      "customer_cannot_mutate_blocks_or_menus",
      "admin_bypasses_draft_status_filter_while_customer_is_restricted_to_published",
      "public_channel_visibility_filters_pages_but_admin_list_bypasses_allowlist",
    ],
  },
  {
    file: "crates/rustok-pages/tests/contract_surface.rs",
    label: "manifest and external builder contract drift tests",
    tokens: [
      "module_manifest_declares_fba_builder_consumer_contract",
      "builder_degraded_modes_bind_to_typed_error_catalog",
      "pages_consumer_version_satisfies_provider_minimum",
    ],
  },
];

function fail(message) {
  console.error("[verify-page-builder-pages-contract-surface] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

for (const check of checks) {
  const filePath = path.join(repoRoot, check.file);
  if (!fs.existsSync(filePath)) {
    fail(`${check.label}: missing file ${check.file}`);
  }
  const content = fs.readFileSync(filePath, "utf8");
  for (const token of check.tokens) {
    if (!content.includes(token)) {
      fail(`${check.label}: ${check.file} missing token '${token}'`);
    }
  }
  console.log(`[verify-page-builder-pages-contract-surface] ${check.label}: PASS`);
}

console.log("[verify-page-builder-pages-contract-surface] PASS");
