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
  label: "rustok-pages capability gates and independent read paths",
  file: "crates/rustok-pages/src/services/page/lifecycle.rs",
  tokens: [
    "ensure_builder_preview_enabled_for_tenant",
    "ensure_builder_properties_enabled_for_tenant",
    "ensure_builder_enabled",
    "PagesError::feature_disabled",
  ],
};

const hostChecks = [
  serviceFallbackCheck,
  {
    label: "rustok-pages reviewed publish capability gate",
    file: "crates/rustok-pages/src/services/page/reviewed_publish.rs",
    tokens: [
      "ensure_builder_publish_enabled_in_tx",
      "is_builder_enabled",
      "is_builder_publish_enabled",
      "PagesError::feature_disabled(\"builder.publish.enabled\")",
    ],
  },
  {
    label: "rustok-pages public storefront read paths",
    file: "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
    tokens: [
      "SecurityContext::public_read()",
      "load_public_bound_artifact_with_fallback",
      "list_public_visible_with_locale_fallback",
      "published_artifact_page_body",
    ],
  },
  {
    label: "rustok-pages public service read paths",
    file: "crates/rustok-pages/src/services/page/read.rs",
    tokens: [
      "pub async fn get_with_locale_fallback",
      "pub async fn list_public_visible_with_locale_fallback",
      "enforce_scope(&security, Resource::Pages, Action::Read)",
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
