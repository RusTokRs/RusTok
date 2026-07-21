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
const sanitizer = read(contract.provider.sanitization.source);
const providerLib = read("crates/rustok-page-builder/src/lib.rs");
const pages = read(contract.pages_consumer.source);
const pagesDto = read(contract.pages_consumer.dto_source);
const pagesErrors = read(contract.pages_consumer.error_source);
const pagesModule = read("crates/rustok-pages/src/services/page/mod.rs");
const pagesServices = read("crates/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/rustok-pages/src/lib.rs");
const receiptEntity = read(contract.pages_consumer.receipt.entity_source);
const receiptMigration = read(contract.pages_consumer.receipt.migration);
const pagesMigrations = read("crates/rustok-pages/src/migrations/mod.rs");
const pagesEntities = read("crates/rustok-pages/src/entities/mod.rs");
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
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) fail(`${label} is missing or out of order at ${marker}`);
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

if (contract.status !== "atomic_service_ready") {
  fail(`unexpected contract status: ${contract.status}`);
}
if (
  contract.provider.context.persisted !== false ||
  contract.provider.sanitization.raw_context_persisted !== false ||
  contract.pages_consumer.raw_context_persisted !== false
) {
  fail("reviewed runtime context must remain transient");
}
if (contract.provider.context.shape !== "json_object") {
  fail("reviewed runtime context must be a JSON object");
}
if (
  contract.provider.review_hash.algorithm !== "sha256" ||
  contract.provider.sanitization.hash_algorithm !== "sha256" ||
  contract.pages_consumer.receipt.hash_algorithm !== "sha256"
) {
  fail("review, sanitization and receipt identities must use sha256");
}
if (contract.provider.scenario.required !== true) {
  fail("reviewed publish runtime must require an explicit scenario");
}
if (
  contract.pages_consumer.builder_sources.required !== true ||
  contract.pages_consumer.builder_sources.ordering !==
    "normalized_locale_ascending"
) {
  fail("atomic reviewed publish must require an ordered Page Builder source set");
}
if (contract.pages_consumer.atomic_pipeline !== "service_integrated") {
  fail("Pages atomic reviewed publish service is not integrated");
}
if (contract.pages_consumer.typed_errors !== true) {
  fail("Pages reviewed publish failures must remain typed");
}
if (contract.pages_consumer.public_transport !== "pending_cutover") {
  fail("public transport status must remain explicit until cutover");
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
  "MAX_PREVIEW_RUNTIME_CONTEXT_BYTES",
  "MAX_PREVIEW_SCENARIO_ID_BYTES",
]) {
  requireMarker(provider, marker, "provider review validation");
}
requireMarker(providerLib, "pub mod publish_runtime;", "provider runtime module export");
requireMarker(providerLib, "PageBuilderReviewedPublishRuntime", "provider runtime export");

requireMarker(
  sanitizer,
  "pub const PAGE_BUILDER_STATIC_SANITIZATION_FORMAT",
  "publish sanitizer format",
);
requireMarker(sanitizer, contract.provider.sanitization.format, "publish sanitizer format");
requireMarker(
  sanitizer,
  `pub struct ${contract.provider.sanitization.envelope}`,
  "publish sanitizer envelope",
);
for (const marker of [
  `pub fn ${contract.provider.sanitization.function}`,
  "StaticLandingCompiler::default().prepare_document(project_data)",
  "serde_json::to_value(document.project)",
  "stable_hash(&sanitized_project)",
  "Sha256::digest(bytes)",
  "result.verify_integrity()",
]) {
  requireMarker(sanitizer, marker, "authoritative publish sanitization");
}
requireMarker(providerLib, "pub mod publish_sanitization;", "sanitizer module export");
requireMarker(providerLib, contract.provider.sanitization.function, "sanitizer public export");

requireMarker(pagesDto, `pub struct ${contract.pages_consumer.input}`, "Pages publish DTO");
requireMarker(pagesDto, `pub struct ${contract.pages_consumer.result}`, "Pages publish result");
for (const field of [
  "expected_version",
  "expected_body_revisions",
  "idempotency_key",
  "runtime",
]) {
  requireMarker(pagesDto, `pub ${field}:`, "Pages publish DTO");
}
requireMarker(pagesDto, "pub struct PageBodyRevisionInput", "Pages body revision DTO");
requireMarker(pagesDto, "pub revision: String", "Pages body revision DTO");

for (const entrypoint of contract.pages_consumer.entrypoints) {
  requireMarker(pages, `pub async fn ${entrypoint}`, "Pages reviewed publish API");
}
if (pages.includes("pub async fn publish_reviewed_if_current")) {
  fail("reviewed publish must expose one atomic service entrypoint");
}
const publishMethod = sliceBetween(
  pages,
  "pub async fn publish_reviewed",
  "fn require_builder_sources",
  "Pages atomic reviewed publish",
);
for (const marker of [
  "normalize_idempotency_key",
  "normalize_expected_body_revisions",
  "find_page_for_update",
  "find_publish_operation_in_tx",
  "publish_result_from_record(operation, true)",
]) {
  requireMarker(publishMethod, marker, "Pages idempotent replay path");
}
requireOrderedMarkers(
  publishMethod,
  contract.pages_consumer.new_operation_order,
  "Pages new publish operation",
);
requireMarker(publishMethod, "txn.commit().await?", "Pages atomic commit");

for (const marker of [
  "type BuilderSourceSet = BTreeMap<String, String>",
  "fn require_builder_sources",
  "atomic reviewed publish requires at least one Page Builder body",
  "fn parse_builder_project_values",
  "compile_builder_sources_with_reviewed_runtime(builder_sources",
]) {
  requireMarker(pages, marker, "shared Page Builder source set");
}

for (const marker of [
  "sanitize_static_landing_project(&project_data)",
  "sanitization_integrity_error",
  "sanitized.project_data()",
  "compile_materialized_static_landing(",
  "artifact_integrity_error",
  "materialized.identity.runtime_scenario_id",
  "materialized.identity.runtime_context_hash",
]) {
  requireMarker(pages, marker, "Pages reviewed materialization");
}
if (pages.includes("PageBuilderPreviewRuntime::default()")) {
  fail("reviewed publish path must not use the default runtime");
}
for (const marker of [
  "RuntimeScenarioReleaseBaseline",
  "baseline.scenarios",
  "&selected.context != &reviewed.context",
  "evaluate_page_builder_runtime_scenario_release",
]) {
  requireMarker(pages, marker, "transactional scenario review gate");
}

requireMarker(
  receiptEntity,
  `table_name = "${contract.pages_consumer.receipt.entity}s"`,
  "publish receipt entity table",
);
for (const field of [
  "id",
  "tenant_id",
  "page_id",
  "idempotency_key",
  ...contract.pages_consumer.receipt.fields,
]) {
  requireMarker(receiptEntity, `pub ${field}:`, "publish receipt entity");
}
for (const marker of [
  "PagePublishOperations::Table",
  "PagePublishOperations::TenantId",
  "PagePublishOperations::PageId",
  "PagePublishOperations::IdempotencyKey",
  ".unique()",
]) {
  requireMarker(receiptMigration, marker, "publish receipt migration");
}
requireMarker(
  pagesMigrations,
  "m20260721_000007_create_page_publish_operations",
  "publish receipt migration registration",
);
requireMarker(
  pagesEntities,
  "pub mod page_publish_operation;",
  "publish receipt entity registration",
);
for (const marker of [
  "insert_publish_operation_in_tx",
  "verify_publish_operation",
  "ensure_same_publish_request",
  "sanitized_set_hash",
  "artifact_set_hash",
]) {
  requireMarker(pages, marker, "publish receipt boundary");
}

for (const marker of contract.pages_consumer.materialization_match) {
  requireMarker(pages, marker, "Pages materialization match");
}
for (const code of contract.pages_consumer.error_codes) {
  requireMarker(pagesErrors, code, "Pages typed error code");
  requireMarker(pagesErrors, `.with_error_code(${code})`, "Pages RichError mapping");
  requireMarker(pagesModule, code, "Pages module error export");
  requireMarker(pagesServices, code, "Pages service error export");
  requireMarker(pagesLib, code, "Pages crate error export");
}
for (const marker of [
  "PublishRuntimeReviewInvalid",
  "PublishSanitize",
  "PublishRuntimeMaterializationMismatch",
  "PublishIdempotencyConflict",
  "PublishOperationIntegrity",
]) {
  requireMarker(pagesErrors, marker, "Pages typed publish error");
}

for (const forbidden of [
  "runtime_context: Set(",
  "raw_runtime_context",
  "publish_runtime_context: Set(",
  "context: Set(",
]) {
  if (
    pages.includes(forbidden) ||
    receiptEntity.includes(forbidden) ||
    receiptMigration.includes(forbidden) ||
    artifactStore.includes(forbidden) ||
    artifactEntity.includes(forbidden)
  ) {
    fail(`raw reviewed runtime context persistence is forbidden: ${forbidden}`);
  }
}

console.log("[verify-page-builder-publish-runtime-review] PASS");
