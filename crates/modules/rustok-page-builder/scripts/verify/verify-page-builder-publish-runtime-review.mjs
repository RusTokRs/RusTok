#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const contract = JSON.parse(
  read("crates/rustok-page-builder/contracts/page-builder-publish-runtime-review.json"),
);
const provider = read(contract.provider.source);
const sanitizer = read(contract.provider.sanitization.source);
const publishPolicy = read(contract.provider.sanitization.policy.source);
const staticLanding = read(contract.provider.sanitization.exact_materialized_recheck_source);
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
const artifactStore = read("crates/rustok-pages/src/services/page_builder_artifact.rs");
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

function forbidMarker(source, marker, label) {
  if (source.includes(marker)) fail(`${label} still contains ${marker}`);
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
if (
  contract.provider.review_hash.algorithm !== "sha256" ||
  contract.provider.sanitization.hash_algorithm !== "sha256" ||
  contract.provider.sanitization.policy.hash_algorithm !== "sha256" ||
  contract.pages_consumer.receipt.hash_algorithm !== "sha256"
) {
  fail("review, policy, sanitization and receipt identities must use sha256");
}
if (
  contract.provider.sanitization.format !==
    "page_builder_static_publish_sanitization_v2" ||
  contract.provider.sanitization.policy.format !==
    "page_builder_static_publish_policy_v1"
) {
  fail("static publish sanitization/policy formats are not current");
}
if (
  JSON.stringify(contract.provider.sanitization.hash_payload) !==
  JSON.stringify([
    "format",
    "policy_format",
    "policy_hash",
    "sanitized_project",
  ])
) {
  fail("sanitization hash must bind policy evidence and the exact sanitized project");
}
if (
  contract.provider.sanitization.policy.enforcement.pre_materialization !== true ||
  contract.provider.sanitization.policy.enforcement.exact_materialized_document !== true ||
  contract.provider.sanitization.policy.failure_mode !== "reject_not_drop"
) {
  fail("static publish policy must fail closed before and after materialization");
}

for (const marker of [
  `pub struct ${contract.provider.dto}`,
  `pub enum ${contract.provider.error}`,
  "pub fn validate(&self)",
  "pub fn preview_runtime(",
  "pub fn runtime_context_hash(",
  "ReviewHashMismatch",
]) {
  requireMarker(provider, marker, "reviewed runtime provider");
}
requireMarker(providerLib, "pub mod publish_runtime;", "provider runtime export");
requireMarker(providerLib, contract.provider.dto, "provider runtime export");

for (const marker of [
  `pub fn ${contract.provider.sanitization.function}`,
  "StaticLandingCompiler::default().prepare_document(project_data)",
  "let PageBuilderStaticPublishPolicyEvidence",
  "validate_static_publish_document(&document)",
  "sanitization_hash(&sanitized_project, &policy_format, &policy_hash)",
  "policy_evidence.verify_integrity()",
  "PAGE_BUILDER_STATIC_SANITIZATION_FORMAT",
  "result.verify_integrity()",
]) {
  requireMarker(sanitizer, marker, "authoritative publish sanitizer");
}

for (const marker of [
  "pub struct PageBuilderStaticPublishPolicy",
  "pub struct PageBuilderStaticPublishPolicyEvidence",
  "pub struct PageBuilderStaticPublishPolicyDiagnostic",
  "pub enum PageBuilderStaticPublishPolicyError",
  "pub fn validate_static_publish_document",
  "ComponentChildren",
  "ComponentNode",
  "validate_component_node(",
  "validate_opaque_node(",
  "validate_style_rules(document, &policy, &mut diagnostics)",
  "validate_assets(document, &policy, &mut diagnostics)",
  "validate_page_metadata(document, &policy, &mut diagnostics)",
  "UrlKind::for_attribute(attribute).is_none()",
  "landing_event_handler_forbidden",
  "landing_false_boolean_attribute_omitted",
  "landing_opaque_node_not_renderable",
  "landing_component_children_opaque",
  "landing_url_rejected",
  "landing_css_value_rejected",
  "landing_style_rule_unbound",
  "landing_style_rule_orphaned",
  "landing_style_rule_empty",
  "landing_asset_url_rejected",
  "landing_metadata_url_rejected",
  "landing_metadata_url_invalid",
  "if values.is_empty()",
  "protocol-relative URLs are forbidden",
]) {
  requireMarker(publishPolicy, marker, "static publish policy");
}
const dangerousTypes = sliceBetween(
  publishPolicy,
  "const DANGEROUS_COMPONENT_TYPES",
  "const FORBIDDEN_ATTRIBUTES",
  "dangerous component type policy",
);
forbidMarker(
  dangerousTypes,
  '"link"',
  "Fly link component must remain publishable",
);

const prepareDocument = sliceBetween(
  staticLanding,
  "pub(crate) fn prepare_document",
  "pub(crate) fn compile_prepared_document",
  "static landing prepare checkpoint",
);
requireOrderedMarkers(
  prepareDocument,
  [
    "document.ensure_stable_ids",
    "require_secure_resource_urls(&document)?;",
    "require_static_publish_policy(&document)?;",
  ],
  "pre-materialization static publish policy order",
);
const compilePrepared = sliceBetween(
  staticLanding,
  "pub(crate) fn compile_prepared_document",
  "pub(crate) fn render_policy",
  "exact materialized compiler checkpoint",
);
requireOrderedMarkers(
  compilePrepared,
  [
    "require_secure_resource_urls(document)?;",
    "require_static_publish_policy(document)?;",
    "build_static_landing_artifact_with_renderer",
  ],
  "exact materialized static publish policy order",
);
for (const marker of [
  "PageBuilderStaticPublishPolicyError::Rejected",
  "landing_static_publish_policy_integrity",
  "ValidationSeverity::Error",
]) {
  requireMarker(staticLanding, marker, "typed static publish policy mapping");
}
requireMarker(providerLib, "pub mod publish_sanitization;", "sanitizer export");
requireMarker(providerLib, "pub mod static_publish_policy;", "publish policy export");
requireMarker(
  providerLib,
  "PageBuilderStaticPublishPolicyEvidence",
  "publish policy evidence export",
);

if (
  contract.pages_consumer.builder_sources.required !== true ||
  contract.pages_consumer.builder_sources.ordering !== "normalized_locale_ascending"
) {
  fail("atomic reviewed publish must require an ordered Page Builder source set");
}
const gateReads = contract.pages_consumer.transactional_gate_reads;
if (
  gateReads?.feature_settings !== "shared_lock_when_supported" ||
  gateReads?.scenario_baseline !== "shared_lock_when_present" ||
  gateReads?.sqlite !== "transaction_serialization"
) {
  fail("transactional feature/scenario gate read policy is invalid");
}
if (
  contract.pages_consumer.atomic_pipeline !==
    "service_and_public_transport_integrated" ||
  contract.pages_consumer.public_transport !==
    "graphql_http_admin_reviewed_cutover"
) {
  fail("reviewed publish service/public transport integration is not recorded");
}
if (contract.pages_consumer.transport.executed_evidence !== "pending") {
  fail("executed transport evidence must remain pending until verification runs");
}

for (const marker of [
  `pub struct ${contract.pages_consumer.input}`,
  `pub struct ${contract.pages_consumer.result}`,
  "pub struct PageBodyRevisionInput",
  "pub expected_version:",
  "pub expected_body_revisions:",
  "pub idempotency_key:",
  "pub runtime:",
]) {
  requireMarker(pagesDto, marker, "Pages atomic publish DTO");
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
  "txn.commit().await?",
]) {
  requireMarker(publishMethod, marker, "Pages idempotent publish path");
}
requireOrderedMarkers(
  publishMethod,
  contract.pages_consumer.new_operation_order,
  "Pages new publish operation",
);
forbidMarker(
  pages,
  "pub async fn publish_reviewed_if_current",
  "reviewed publish service",
);
forbidMarker(
  pages,
  "PageBuilderPreviewRuntime::default()",
  "reviewed publish service",
);
for (const marker of [
  "type BuilderSourceSet = BTreeMap<String, String>",
  "atomic reviewed publish requires at least one Page Builder body",
  "sanitize_static_landing_project(&project_data)",
  "compile_materialized_static_landing(",
  "materialized.identity.runtime_scenario_id",
  "materialized.identity.runtime_context_hash",
  "query().lock_shared().one(txn).await?",
]) {
  requireMarker(pages, marker, "Pages reviewed publish invariant");
}

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

for (const code of contract.pages_consumer.error_codes) {
  requireMarker(pagesErrors, code, "Pages typed publish error code");
  requireMarker(pagesErrors, `.with_error_code(${code})`, "Pages RichError mapping");
  requireMarker(pagesModule, code, "Pages module error export");
  requireMarker(pagesServices, code, "Pages service error export");
  requireMarker(pagesLib, code, "Pages crate error export");
}

requireMarker(
  JSON.stringify(contract.pages_consumer.rollback),
  "hash_match_without_activation_receipt",
  "rollback activation contract",
);

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
