#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-native-storefront-reviewed-artifact-source.json",
  ),
);
const cargo = read("crates/rustok-pages/storefront/Cargo.toml");
const harness = read(
  "crates/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs",
);
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const reviewedPublish = read(
  "crates/rustok-pages/src/services/page/reviewed_publish.rs",
);
const artifactService = read(
  "crates/rustok-pages/src/services/page_builder_artifact.rs",
);
const packet = read(
  "docs/modules/pages-page-builder-native-storefront-reviewed-artifact-packet-2026-08-05.md",
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
  visible_channel_returned_fly_artifact_url: true,
  artifact_url_retained_selected_channel: true,
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
    "crates/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs" ||
  evidence.harness?.test !==
    "native_storefront_returns_reviewed_artifact_for_visible_channel_and_refuses_unverified_fill" ||
  evidence.harness?.route !== "/api/fn/pages/storefront-data"
) {
  failures.push("reviewed artifact harness registration is invalid");
}

requireText(
  cargo,
  "rustok-page-builder.workspace = true",
  "reviewed artifact dev dependency",
);
for (const marker of [
  '#![cfg(feature = "ssr")]',
  "use rustok_pages_storefront as _;",
  "handle_server_fns_with_context",
  "TenantContextExtension(tenant.clone())",
  "ChannelContextExtension(channel.clone())",
  "PageBuilderReviewedPublishRuntime::new(",
  ".publish_reviewed(",
  "expected_version: draft.version",
  "PageBodyRevisionInput",
  "ReviewedPagePublishRuntimeInput",
  "assert!(artifact.materialization_hash.is_some())",
  "assert!(artifact.materialization_identity.is_some())",
  "assert!(artifact.runtime_snapshots.is_some())",
  'create_enabled_channel(&channel_service, tenant_id, "web", "Web")',
  'create_enabled_channel(&channel_service, tenant_id, "mobile", "Mobile")',
  "assert!(visible.body.contains(\"fly_artifact_url\"))",
  "assert!(visible.body.contains(&fixture.expected_artifact_url))",
  "assert!(!hidden.body.contains(\"fly_artifact_url\"))",
  "assert_ne!(hidden_cache.get_keys[0], hidden_cache.get_keys[1])",
  "corrupt_artifact_document(&db, fixture.artifact_id).await?",
  "assert_ne!(corrupt.status, StatusCode::OK)",
  "after_corrupt_read.put_keys",
  "before_corrupt_read.put_keys",
  "PAGES_STOREFRONT_CACHE_TTL_SECS",
]) {
  requireText(harness, marker, "reviewed artifact route harness");
}

const routeTest = between(
  harness,
  "async fn native_storefront_returns_reviewed_artifact_for_visible_channel_and_refuses_unverified_fill(",
  "fn native_server_fn_router(",
  "reviewed artifact route test",
);
requireOrder(
  routeTest,
  [
    "let visible = call_storefront",
    "assert_eq!(visible.status, StatusCode::OK)",
    "let visible_cache = cache.snapshot()",
    "let hidden = call_storefront",
    "assert_eq!(hidden.status, StatusCode::OK)",
    "let hidden_cache = cache.snapshot()",
    "corrupt_artifact_document",
    "let before_corrupt_read = cache.snapshot()",
    "let corrupt = call_storefront",
    "assert_ne!(corrupt.status, StatusCode::OK)",
    "let after_corrupt_read = cache.snapshot()",
    "after_corrupt_read.put_keys",
    "before_corrupt_read.put_keys",
  ],
  "visible hidden corrupt ordering",
);

const publishOwner = between(
  reviewedPublish,
  "pub async fn publish_reviewed(",
  "fn require_builder_sources(",
  "reviewed publish owner",
);
requireOrder(
  publishOwner,
  [
    "normalize_expected_body_revisions",
    "input.runtime.try_into()",
    "enforce_expected_version",
    "load_bodies_for_reviewed_publish",
    "ensure_builder_publish_enabled_in_tx",
    "ensure_candidates_allowed_in_tx",
    "compile_builder_sources_with_reviewed_runtime",
    "PageBuilderArtifactService::stage_compiled_in_tx",
    "PageBuilderArtifactService::bind_existing_body_in_tx",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "insert_publish_operation_in_tx",
    "txn.commit().await?",
  ],
  "reviewed publish transaction ordering",
);
const compileOwner = between(
  reviewedPublish,
  "fn compile_builder_sources_with_reviewed_runtime(",
  "async fn ensure_builder_publish_enabled_in_tx(",
  "reviewed compile owner",
);
requireOrder(
  compileOwner,
  [
    "reviewed.validate()",
    "reviewed.preview_runtime()",
    "sanitize_static_landing_project",
    ".verify_integrity()",
    "compile_materialized_static_landing",
    ".verify_integrity()",
    "materialization_hash",
    "materialization_identity",
    "runtime_snapshots",
  ],
  "sanitization materialization ordering",
);

const nativeOwner = between(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "native storefront owner",
);
requireOrder(
  nativeOwner,
  [
    "is_module_enabled(channel_id, MODULE_SLUG)",
    "generation_snapshot(tenant_id).await",
    "get_json::<StorefrontPagesData>(cache_key)",
    "get_by_slug_with_locale_fallback(",
    "is_visible_for_public_channel",
    'body.format.eq_ignore_ascii_case("grapesjs")',
    "PageBuilderArtifactService::new(runtime_ctx.db_clone())",
    ".load_public_bound_artifact_with_fallback(",
    "published_artifact_page_body(",
    "put_json(cache_key, &data).await",
  ],
  "native artifact selection before fill ordering",
);
for (const marker of [
  'format: "fly_artifact_url".to_string()',
  'query.push_str("&channel=")',
  'format!("/api/pages/{page_id}/artifact?{query}")',
]) {
  requireText(nativeAdapter, marker, "native artifact URL composition");
}

const artifactOwner = between(
  artifactService,
  "pub async fn load_public_bound_artifact_with_fallback(",
  "async fn find_artifact_in_tx(",
  "public bound artifact owner",
);
requireOrder(
  artifactOwner,
  [
    'page.status == "published"',
    "page_is_visible_for_channel_in_tx",
    "load_bound_artifact_in_tx",
    "page_published_landing_artifact::Entity::find_by_id(body.id)",
    "page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)",
    "published_record(record).map(Some)",
  ],
  "visibility binding integrity ordering",
);
requireText(artifactService, "verify_record(&record)?;", "artifact verification");
requireText(
  artifactService,
  "PageBuilderMaterializedStaticLandingArtifact",
  "materialization envelope reconstruction",
);

for (const marker of [
  "Reviewed publication fixture",
  "Visible channel and immutable artifact URL",
  "Hidden-channel isolation",
  "Integrity failure cannot fill a fresh key",
  "A cache value cannot be created from an unverified immutable artifact",
]) {
  requireText(packet, marker, "reviewed artifact packet");
}
for (const marker of [
  "native-storefront-reviewed-artifact-source-ready",
  "Native reviewed immutable artifact selection: source-ready",
  "full Page Builder materialization envelope",
  "durable `NodePublished`",
  "registered native storefront miss/refill",
]) {
  requireText(continuation, marker, "parity continuation plan");
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
