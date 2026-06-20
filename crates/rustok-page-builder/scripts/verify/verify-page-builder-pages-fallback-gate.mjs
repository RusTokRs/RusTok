#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function fail(message) {
  console.error("[verify-page-builder-pages-fallback-gate] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

const serviceFallbackCheck = {
  label: "rustok-pages service fallback profile tests",
  file: "crates/rustok-pages/tests/page_service_kind_guard.rs",
  tokens: [
    "pages_builder_fallback_all_on_allows_publish_and_keeps_read_list_paths",
    "pages_builder_fallback_publish_off_blocks_grapesjs_publish_but_keeps_read_list_paths",
    "pages_builder_fallback_preview_off_blocks_preview_publish_but_keeps_read_list_paths",
    "pages_builder_fallback_builder_off_keeps_read_and_list_paths",
    "FeatureDisabled",
    ".list(tenant_id",
    ".get(tenant_id",
  ],
};

const hostChecks = [
  serviceFallbackCheck,
  {
    label: "rustok-pages-admin host fallback helpers",
    file: "crates/rustok-pages/admin/src/core.rs",
    tokens: [
      "builder_host_fallback_surface",
      "editable_builder",
      "editable_builder_publish_disabled",
      "preview_hidden_properties_available",
      "readonly_fallback",
      "feature-disabled",
      "builder_host_fallback_profiles_keep_read_list_stable",
    ],
  },
  {
    label: "rustok-pages-storefront host fallback helpers",
    file: "crates/rustok-pages/storefront/src/core.rs",
    tokens: [
      "storefront_builder_fallback_read_contract",
      "read_paths_stable: true",
      "list_paths_stable: true",
      "render_requires_builder_capability: false",
      "storefront_builder_fallback_profiles_keep_read_and_list_stable",
    ],
  },
];

for (const check of hostChecks) {
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
  console.log(`[verify-page-builder-pages-fallback-gate] ${check.label}: PASS`);
}

console.log("[verify-page-builder-pages-fallback-gate] PASS");
