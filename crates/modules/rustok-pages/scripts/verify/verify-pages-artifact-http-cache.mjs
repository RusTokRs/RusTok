#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read("crates/rustok-pages/contracts/evidence/pages-artifact-http-cache-source.json"),
);
const cargo = read("crates/rustok-pages/Cargo.toml");
const harness = read("crates/rustok-pages/tests/artifact_http_cache_sqlite.rs");
const controller = read("crates/rustok-pages/src/controllers/mod.rs");
const artifactOwner = read(
  "crates/rustok-pages/src/services/page_builder_artifact.rs",
);
const cacheContract = read("crates/rustok-pages/src/cache_invalidation.rs");
const compiler = read("crates/rustok-page-builder/src/static_landing.rs");
const overlay = read(
  "docs/modules/pages-page-builder-artifact-http-cache-packet-2026-08-04.md",
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sliceBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return content.slice(startIndex, endIndex);
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

if (evidence.status !== "pages_artifact_http_cache_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("executed evidence must remain empty");
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "sqlite_run",
  "artifact_http_run",
  "cache_miss_observed",
  "owner_read_observed",
  "cache_refill_observed",
  "cache_hit_observed",
  "conditional_304_observed",
  "storefront_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  sqlite_router_harness_added: true,
  real_pages_module_migrations_used: true,
  valid_page_builder_static_artifact_compiled: true,
  public_pages_axum_router_used: true,
  host_runtime_context_used: true,
  typed_pages_cache_runtime_used: true,
  tenant_context_extension_used: true,
  initial_artifact_generation_positive: true,
  first_request_returns_200: true,
  first_request_misses_cache: true,
  first_request_loads_owner_binding: true,
  first_request_refills_cache: true,
  first_response_retains_html_and_security_headers: true,
  binding_removed_before_first_hit: true,
  second_request_returns_304: true,
  second_request_hits_old_generation_cache: true,
  second_request_does_not_refill: true,
  artifact_generation_advanced: true,
  new_generation_key_differs: true,
  old_generation_value_remains_present: true,
  binding_restored_before_new_generation_miss: true,
  third_request_returns_200: true,
  third_request_misses_new_generation_key: true,
  third_request_loads_owner_binding: true,
  third_request_refills_new_generation_key: true,
  binding_removed_before_new_generation_hit: true,
  fourth_request_returns_304: true,
  fourth_request_hits_new_generation_cache: true,
  conditional_304_body_empty: true,
  etag_stable_across_generation_change: true,
  cache_ttl_contract_retained: true,
  test_only_tower_dependency_added: true,
  production_artifact_behavior_changed: false,
  production_cache_behavior_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  storefront_server_function_executed: false,
  postgres_executed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
    "crates/rustok-pages/tests/artifact_http_cache_sqlite.rs" ||
  evidence.harness?.test !==
    "artifact_http_misses_refills_hits_and_returns_conditional_304_across_generation_change" ||
  evidence.harness?.backend !== "sqlite_in_memory" ||
  evidence.harness?.route !== "/api/pages/{id}/artifact?locale=en"
) {
  failures.push("artifact HTTP harness registration is invalid");
}

const dependencies = sliceBetween(
  cargo,
  "[dependencies]",
  "[dev-dependencies]",
  "Pages dependencies",
);
const devDependencies = cargo.slice(cargo.indexOf("[dev-dependencies]"));
forbidText(dependencies, "tower.workspace = true", "production dependency boundary");
requireText(devDependencies, "tower.workspace = true", "test router dependency");

for (const marker of [
  "struct RecordingCachePort",
  "impl PagesCacheReadPort for RecordingCachePort",
  "RecordingCachePort::new(PageCacheGenerationSnapshot::new(\n        3, 5, 7,\n    ))",
  "HostRuntimeContext::new(db.clone())",
  ".with_shared_value(mock_transactional_event_bus())",
  ".with_shared_value(PagesCacheReadRuntime::new(cache_port))",
  "controllers::axum_router(&host)?",
  "StaticLandingCompiler::default().compile_publish(&project)?",
  "artifact.verify_integrity()?",
  "for migration in PagesModule.migrations()",
  "TenantContextExtension(tenant_context(fixture.tenant_id))",
  "tower::ServiceExt",
  "Duration::from_secs(60)",
  "assert!(csp.contains(\"style-src 'sha256-\"))",
]) {
  requireText(harness, marker, "artifact HTTP SQLite harness");
}

const testBody = sliceBetween(
  harness,
  "async fn artifact_http_misses_refills_hits_and_returns_conditional_304_across_generation_change(",
  "async fn setup_db()",
  "artifact HTTP cache test",
);
requireOrder(
  testBody,
  [
    "let first =",
    "assert_eq!(first.status(), StatusCode::OK)",
    "assert_eq!(first_cache.get_keys.len(), 1)",
    "assert_eq!(first_cache.put_keys.len(), 1)",
    "let old_generation_key = first_cache.put_keys[0].clone()",
    "delete_binding(&db, fixture.body_id).await?",
    "let second =",
    "assert_eq!(second.status(), StatusCode::NOT_MODIFIED)",
    "assert_eq!(second_cache.put_keys.len(), 1)",
    "insert_binding(&db, &fixture).await?",
    "cache.set_artifact_generation(8)",
    "let third =",
    "assert_eq!(third.status(), StatusCode::OK)",
    "assert_eq!(third_cache.put_keys.len(), 2)",
    "let new_generation_key = third_cache.put_keys[1].clone()",
    "assert_ne!(new_generation_key, old_generation_key)",
    "assert!(third_cache.keys.contains(&old_generation_key))",
    "delete_binding(&db, fixture.body_id).await?",
    "let fourth =",
    "assert_eq!(fourth.status(), StatusCode::NOT_MODIFIED)",
    "assert_eq!(fourth_cache.put_keys.len(), 2)",
    "assert_eq!(fourth_cache.get_keys[3], new_generation_key)",
  ],
  "HTTP miss refill hit generation ordering",
);
for (const marker of [
  "to_bytes(second.into_body(), RESPONSE_BODY_LIMIT)\n            .await?\n            .is_empty()",
  "to_bytes(fourth.into_body(), RESPONSE_BODY_LIMIT)\n            .await?\n            .is_empty()",
  "artifact_request(&fixture, Some(&first_etag))",
]) {
  requireText(testBody, marker, "conditional artifact response evidence");
}

const artifactHandler = sliceBetween(
  controller,
  "pub async fn get_page_artifact(",
  "async fn load_cached_page_artifact(",
  "production artifact HTTP handler",
);
requireOrder(
  artifactHandler,
  [
    "ensure_pages_module_enabled_for_channel(&runtime, &request_context).await?",
    "load_cached_page_artifact(",
    "let etag = format!",
    "header::IF_NONE_MATCH",
    "StatusCode::NOT_MODIFIED",
    ".body(Body::empty())",
    "StatusCode::OK",
    ".body(Body::from(artifact.document_html))",
  ],
  "production conditional response ordering",
);
for (const marker of [
  "header::CONTENT_TYPE",
  "header::CONTENT_LANGUAGE",
  "header::ETAG",
  "header::VARY",
  "header::CACHE_CONTROL",
  'header("content-security-policy", csp)',
  'header("referrer-policy", "strict-origin-when-cross-origin")',
  'header("x-content-type-options", "nosniff")',
  'header("cross-origin-resource-policy", "same-origin")',
]) {
  requireText(artifactHandler, marker, "production artifact response contract");
}

const cacheRead = sliceBetween(
  controller,
  "async fn load_cached_page_artifact(",
  "fn artifact_cache_variant(",
  "production artifact cache read",
);
requireOrder(
  cacheRead,
  [
    "generation_snapshot(tenant_id).await",
    "page_cache_key(",
    "PageCacheScope::Artifact",
    "get_json::<CachedPublishedLandingArtifact>(cache_key)",
    "load_public_bound_artifact_with_fallback(",
    "put_json(cache_key, &artifact).await",
  ],
  "generation cache owner refill ordering",
);

const ownerRead = sliceBetween(
  artifactOwner,
  "pub async fn load_public_bound_artifact_with_fallback(",
  "async fn page_is_visible_for_channel_in_tx(",
  "production artifact owner read",
);
requireOrder(
  ownerRead,
  [
    "let txn = self.db.begin().await?",
    'page.status == "published"',
    "page_is_visible_for_channel_in_tx",
    "build_locale_candidates(",
    "load_bound_artifact_in_tx(",
    "txn.commit().await?",
  ],
  "owner publication visibility binding ordering",
);
const boundRead = sliceBetween(
  artifactOwner,
  "async fn load_bound_artifact_in_tx(",
  "async fn find_canonical_artifact_in_tx(",
  "published artifact binding read",
);
requireOrder(
  boundRead,
  [
    "page_body::Entity::find()",
    "page_published_landing_artifact::Entity::find_by_id(body.id)",
    "page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)",
    "published_record(record).map(Some)",
  ],
  "body binding artifact read ordering",
);
for (const marker of [
  "fn published_record(",
  "verify_record(&record)?",
  "fn verify_record(",
  ".verify_integrity()",
]) {
  requireText(artifactOwner, marker, "artifact integrity owner");
}

for (const marker of [
  "pub trait PagesCacheReadPort",
  "pub struct PagesCacheReadRuntime",
  "validate_cache_value_size(bytes.len())?",
  "Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS)",
]) {
  requireText(cacheContract, marker, "Pages cache read contract");
}
for (const marker of [
  "pub struct StaticLandingCompiler",
  "pub fn compile_publish(",
  "build_static_landing_artifact_with_renderer(",
]) {
  requireText(compiler, marker, "Page Builder static compiler");
}

for (const marker of [
  "Artifact HTTP cache packet: ready, unvalidated",
  "old-generation cache value",
  "artifact generation advances from `7` to `8`",
  "SQLite and Axum execution remain pending",
  "native storefront server-function packet remains open",
]) {
  requireText(overlay, marker, "artifact HTTP continuation overlay");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageCacheInvalidationPort",
]) {
  forbidText(harness, forbidden, "artifact HTTP harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-http-cache] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-artifact-http-cache] PASS source_ready=true sqlite_execution=pending",
);
