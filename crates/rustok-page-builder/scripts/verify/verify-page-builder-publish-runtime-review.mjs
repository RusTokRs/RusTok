#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const contract = JSON.parse(
  read(
    "crates/rustok-page-builder/contracts/page-builder-publish-runtime-review.json",
  ),
);
const provider = read(contract.provider.source);
const providerLib = read("crates/rustok-page-builder/src/lib.rs");
const pages = read(contract.pages_consumer.source);
const pagesModule = read("crates/rustok-pages/src/services/page/mod.rs");
const pagesServices = read("crates/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/rustok-pages/src/lib.rs");
const artifactStore = read(
  "crates/rustok-pages/src/services/page_builder_artifact.rs",
);
const artifactEntity = read(
  "crates/rustok-pages/src/entities/page_static_landing_artifact.rs",
);

function fail(message) {
  console.error(`[verify-page-builder-publish-runtime-review] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function requireOrderedMarkers(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) fail(`${label} is missing ${marker}`);
    if (index <= previous) fail(`${label} is out of order at ${marker}`);
    previous = index;
  }
}

function sliceBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) fail(`${label} is missing ${start}`);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) fail(`${label} is missing ${end}`);
  return source.slice(startIndex, endIndex);
}

if (contract.status !== "domain_service_ready") {
  fail(`unexpected contract status: ${contract.status}`);
}
if (contract.provider.context.persisted !== false) {
  fail("reviewed runtime context must remain transient");
}
if (contract.provider.context.shape !== "json_object") {
  fail("reviewed runtime context must be a JSON object");
}
if (contract.provider.review_hash.algorithm !== "sha256") {
  fail("reviewed runtime hash algorithm must be sha256");
}
if (contract.provider.scenario.required !== true) {
  fail("reviewed publish runtime must require an explicit scenario");
}
if (contract.pages_consumer.raw_context_persisted !== false) {
  fail("Pages must not persist raw reviewed runtime context");
}

requireMarker(provider, `pub struct ${contract.provider.dto}`, "provider DTO");
requireMarker(provider, `pub enum ${contract.provider.error}`, "provider error");
requireMarker(provider, contract.format, "provider format");
for (const field of contract.provider.fields) {
  requireMarker(provider, `pub ${field}:`, "provider DTO");
}
for (const marker of [
  "pub fn validate(&self)",
  "pub fn preview_runtime(",
  "pub fn runtime_context_hash(",
  "self.context.is_object()",
  "serde_json::to_vec(&self.context)",
  "Sha256::digest(bytes)",
  "ReviewHashMismatch",
]) {
  requireMarker(provider, marker, "provider review validation");
}
requireMarker(
  provider,
  "MAX_PREVIEW_RUNTIME_CONTEXT_BYTES",
  "provider context limit",
);
requireMarker(
  provider,
  "MAX_PREVIEW_SCENARIO_ID_BYTES",
  "provider scenario limit",
);
requireMarker(providerLib, "pub mod publish_runtime;", "provider module export");
requireMarker(
  providerLib,
  "PageBuilderReviewedPublishRuntime",
  "provider public export",
);

for (const entrypoint of contract.pages_consumer.entrypoints) {
  requireMarker(pages, `pub async fn ${entrypoint}`, "Pages reviewed publish API");
}
const preflight = sliceBetween(
  pages,
  "pub async fn publish_reviewed_if_current",
  "async fn transition_page_with_reviewed_runtime",
  "Pages reviewed publish preflight",
);
requireOrderedMarkers(
  preflight,
  contract.pages_consumer.required_order,
  "Pages reviewed publish preflight",
);
const transaction = sliceBetween(
  pages,
  "async fn transition_page_with_reviewed_runtime",
  "fn compile_builder_sources_with_reviewed_runtime",
  "Pages reviewed publish transaction",
);
requireOrderedMarkers(
  transaction,
  contract.pages_consumer.transaction_order,
  "Pages reviewed publish transaction",
);

for (const marker of contract.pages_consumer.materialization_match) {
  requireMarker(pages, marker, "Pages materialization match");
}
for (const code of contract.pages_consumer.error_codes) {
  requireMarker(pages, code, "Pages stable error code");
  requireMarker(pagesModule, code, "Pages module error export");
  requireMarker(pagesServices, code, "Pages service error export");
  requireMarker(pagesLib, code, "Pages crate error export");
}
for (const marker of [
  "reviewed.preview_runtime()",
  ".runtime_context_hash()",
  "compile_materialized_static_landing(&project_data, runtime.clone())",
  ".verify_integrity()",
  "materialized.identity.runtime_scenario_id",
  "materialized.identity.runtime_context_hash",
]) {
  requireMarker(pages, marker, "Pages reviewed materialization");
}
if (pages.includes("PageBuilderPreviewRuntime::default()")) {
  fail("reviewed publish path must not use the default runtime");
}

for (const forbidden of [
  "runtime_context: Set(",
  "raw_runtime_context",
  "publish_runtime_context: Set(",
]) {
  if (
    pages.includes(forbidden) ||
    artifactStore.includes(forbidden) ||
    artifactEntity.includes(forbidden)
  ) {
    fail(`raw reviewed runtime context persistence is forbidden: ${forbidden}`);
  }
}

console.log("[verify-page-builder-publish-runtime-review] PASS");
