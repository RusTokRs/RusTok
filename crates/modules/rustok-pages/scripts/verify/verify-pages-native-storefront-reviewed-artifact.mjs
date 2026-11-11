#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/modules/rustok-pages/contracts/evidence/pages-native-storefront-reviewed-artifact-source.json",
  ),
);
const cargo = read("crates/modules/rustok-pages/storefront/Cargo.toml");
const harness = read(
  "crates/modules/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs",
);
const nativeAdapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const reviewedPublish = read(
  "crates/modules/rustok-pages/src/services/page/reviewed_publish.rs",
);
const artifactService = read(
  "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
);
const packet = read(
);
const continuation = read(
  "docs/modules/pages-page-builder-parity-continuation-plan.md",
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};

if (
  evidence.status !==
  "pages_native_storefront_reviewed_artifact_source_unvalidated"
) {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const [key, expected] of Object.entries({
  real_pages_reviewed_publish_used: true,
  page_builder_review_runtime_used: true,
  page_builder_authoritative_sanitization_reached_through_owner: true,
  page_builder_runtime_materialization_reached_through_owner: true,
  full_materialization_hash_retained: true,
  full_materialization_identity_retained: true,
  runtime_snapshots_retained: true,
  visible_channel_returned_published_landing: true,
  authoring_document_absent_from_public_response: true,
  hidden_channel_did_not_select_artifact: true,
  channel_variants_used_distinct_cache_keys: true,
  corrupted_materialization_record_rejected: true,
  corrupted_materialization_record_not_cached: true,
  cache_fill_follows_artifact_integrity: true,
  production_storefront_behavior_changed: false,
  production_page_builder_behavior_changed: false,
  production_database_schema_changed: false,
  node_published_relay_continuity_executed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
    "crates/modules/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs" ||
  evidence.harness?.test !==
    "native_storefront_returns_reviewed_artifact_for_visible_channel_and_refuses_unverified_fill" ||
  evidence.harness?.route !== "/api/fn/pages/storefront-data"
) {
  failures.push("reviewed artifact harness registration is invalid");
}

requireText(
  cargo,
  "rustok-page-builder",
  "reviewed artifact dev dependency",
);
, marker, "reviewed artifact packet");
}
, marker, "parity continuation plan");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageCacheInvalidationPort",
  "publish_non_builder",
]) {
  forbidText(harness, forbidden, "reviewed artifact harness boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-reviewed-artifact] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-reviewed-artifact] PASS source_ready=true execution=pending",
);
