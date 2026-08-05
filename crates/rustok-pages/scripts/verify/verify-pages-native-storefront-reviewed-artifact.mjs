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

if (
  evidence.status !==
  "pages_native_storefront_reviewed_artifact_source_unvalidated"
) {
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
  "axum_run",
  "leptos_server_fn_run",
  "reviewed_publish_observed",
  "sanitization_observed",
  "materialization_observed",
  "artifact_integrity_observed",
  "channel_selection_observed",
  "cache_fill_observed",
  "corrupt_fill_rejection_observed",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  real_leptos_server_function_registry_used: true,
  production_server_function_endpoint_used: true,
  production_server_function_handler_shape_used: true,
  host_runtime_context_used: true,
  trusted_tenant_extension_used: true,
  trusted_channel_extension_used: true,
  request_context_extracted_by_production_adapter: true,
  real_outbox_migration_used: true,
  real_channel_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_channel_owner_create_used: true,
  real_channel_owner_module_binding_used: true,
  real_pages_owner_create_used: true,
  real_pages_reviewed_publish_used: true,
  page_builder_review_runtime_used: true,
  page_builder_authoritative_sanitization_reached_through_owner: true,
  page_builder_runtime_materialization_reached_through_owner: true,
  durable_publish_receipt_retained: true,
  full_materialization_hash_retained: true,
  full_materialization_identity_retained: true,
  runtime_snapshots_retained: true,
  typed_pages_cache_runtime_used: true,
  visible_channel_selected_reviewed_page: true,
  visible_channel_returned_fly_artifact_url: true,
  artifact_url_retained_selected_channel: true,
  hidden_channel_did_not_select_artifact: true,
  channel_variants_used_distinct_cache_keys: true,
  verified_owner_response_filled_cache: true,
  production_storefront_ttl_used: true,
  corrupted_materialization_record_rejected: true,
  corrupted_materialization_record_not_cached: true,
  cache_fill_follows_artifact_integrity: true,
  production_storefront_behavior_changed: false,
  production_page_builder_behavior_changed: false,
  production_cache_policy_changed: false,
  production_database_schema_changed: false,
  public_route_changed: false,
  node_published_relay_continuity_executed: false,
  postgres_executed: false,
  browser_executed: false,
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
  evidence.harness?.backend !== "sqlite_in_memory" ||
  evidence.harness?.route !== "/api/fn/pages/storefront-data" ||
  evidence.harness?.codec !== "application/x-www-form-urlencoded"
) {
  failures.push("reviewed artifact harness registration is invalid");
}

requireText(
  cargo,
  "rustok-page-builder.workspace = true",
  "storefront reviewed artifact dev dependency",
);
for (const marker of [
  '#![cfg(feature = "ssr")]',
  "use rustok_pages_storefront as _;",
  "PageBuilderReviewedPublishRuntime",
  "handle_server_fns_with_context",
  "provide_context(host.clone())",
  'const SERVER_FN_PATH: &str = "/api/fn/pages/storefront-data"',
  'header::CONTENT_TYPE, "application/x-www-form-urlencoded"',
  "TenantContextExtension(tenant.clone())",
  "ChannelContextExtension(channel.clone())",
  "HostRuntimeContext::new(db.clone())",
  ".with_shared_value(event_bus)",
  ".with_shared_value(PagesCacheReadRuntime::new(cache_port))",
  "SysEventsMigration.up(&manager).await?",
  "for migration in ChannelModule.migrations()",
  "for migration in PagesModule.migrations()",
  "PageService::new(db.clone(), event_bus)",
  ".create(",
  "CONTENT_FORMAT_GRAPESJS",
  "channel_slugs: Some(vec![\"web\".to_string()])",
  "PageBuilderReviewedPublishRuntime::new(",
  ".publish_reviewed(",
  "PageBodyRevisionInput",
  "ReviewedPagePublishRuntimeInput",
  "assert_eq!(publish.review_hash.len(), 64)",
  "assert_eq!(publish.sanitized_set_hash.len(), 64)",
  "assert_eq!(publish.artifact_set_hash.len(), 64)",
  "assert!(artifact.materialization_hash.is_some())",
  "assert!(artifact.materialization_identity.is_some())",
  "assert!(artifact.runtime_snapshots.is_some())",
  "ChannelService::new(db.clone())",
  "create_enabled_channel(&channel_service, tenant_id, \"web\", \"Web\")",
  "create_enabled_channel(&channel_service, tenant_id, \"mobile\", \"Mobile\")",
  "PAGES_STOREFRONT_CACHE_TTL_SECS",
  "corrupt_artifact_document(&db, fixture.artifact_id).await?",
]) {
  requireText(harness, marker, "reviewed artifact registered-route harness");
}

const testBody = sliceBetween(
  harness,
  "async fn native_storefront_returns_reviewed_artifact_for_visible_channel_and_refuses_unverified_fill(",
  "fn native_server_fn_router(",
  "reviewed artifact route test",
);
requireOrder(
  testBody,
  [
    "create_reviewed_published_page",
    "create_enabled_channel(&channel_service, tenant_id, \"web\", \"Web\")",
    "create_enabled_channel(&channel_service, tenant_id, \"mobile\", \"Mobile\")",
    "let visible = call_storefront",
    "assert_eq!(visible.status, StatusCode::OK)",
    "assert!(visible.body.contains(\"fly_artifact_url\"))",
    "assert!(visible.body.contains(&fixture.expected_artifact_url))",
    "assert_eq!(visible_cache.generation_reads, 1)",
    "assert_eq!(visible_cache.put_keys.len(), 1)",
    "let hidden = call_storefront",
    "assert_eq!(hidden.status, StatusCode::OK)",
    "assert!(!hidden.body.contains(&fixture.expected_artifact_url))",
    "assert!(!hidden.body.contains(\"fly_artifact_url\"))",
    "assert_ne!(hidden_cache.get_keys[0], hidden_cache.get_keys[1])",
    "corrupt_artifact_document",
    "let before_corrupt_read = cache.snapshot()",
    "let corrupt = call_storefront",
    "assert_ne!(corrupt.status, StatusCode::OK)",
    "contains(\"integrity\")",
    "after_corrupt_read.generation_reads",
    "before_corrupt_read.generation_reads + 1",
    "after_corrupt_read.get_keys.len()",
    "before_corrupt_read.get_keys.len() + 1",
    "after_corrupt_read.put_keys",
    "before_corrupt_read.put_keys",
    "after_corrupt_read.key_count",
    "before_corrupt_read.key_count",
  ],
  "visible hidden corrupt route ordering",
);

const publishFixture = sliceBetween(
  harness,
  "async fn create_reviewed_published_page(",
  "async fn create_enabled_channel(",
  "reviewed publication fixture",
);
requireOrder(
  publishFixture,
  [
    "PageService::new",
    ".create(",
    "updated_at",
    "PageBuilderReviewedPublishRuntime::new(",
    ".publish_reviewed(",
    "expected_version: draft.version",
    "expected_body_revisions",
    "idempotency_key",
    "ReviewedPagePublishRuntimeInput",
    "assert!(!publish.replayed)",
    "page_static_landing_artifact::Entity::find()",
    "assert!(artifact.materialization_hash.is_some())",
    "assert!(artifact.materialization_identity.is_some())",
    "assert!(artifact.runtime_snapshots.is_some())",
  ],
  "reviewed owner publication ordering",
);

const nativeBody = sliceBetween(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "production native storefront adapter",
);
requireOrder(
  nativeBody,
  [
    "is_module_enabled(channel_id, MODULE_SLUG)",
    "let cache_variant = storefront_cache_variant(",
    "generation_snapshot(tenant_id).await",
    "storefront_pages_cache_key(",
    "get_json::<StorefrontPagesData>(cache_key)",
    "let service = PageService::new",
    "get_by_slug_with_locale_fallback(",
    "is_visible_for_public_channel",
    'body.format.eq_ignore_ascii_case("grapesjs")',
    "PageBuilderArtifactService::new(runtime_ctx.db_clone())",
    ".load_public_bound_artifact_with_fallback(",
    "published_artifact_page_body(",
    "list_public_visible(",
    "put_json(cache_key, &data).await",
  ],
  "production native reviewed artifact ordering",
);
for (const marker of [
  'format: "fly_artifact_url".to_string()',
  'format!("/api/pages/{page_id}/artifact?{query}")',
  'query.push_str("&channel=")',
]) {
  requireText(nativeAdapter, marker, "production artifact URL adapter");
}

const reviewedOwner = sliceBetween(
  reviewedPublish,
  "pub async fn publish_reviewed(",
  "fn require_builder_sources(",
  "Pages reviewed publish owner",
);
requireOrder(
  reviewedOwner,
  [
    "normalize_expected_body_revisions",
    "input.runtime.try_into()",
    "find_page_for_update",
    "enforce_expected_version",
    "load_bodies_for_reviewed_publish",
    "current_revisions != expected_body_revisions",
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
  "Pages reviewed publish transaction ordering",
);
const compiler = sliceBetween(
  reviewedPublish,
  "fn compile_builder_sources_with_reviewed_runtime(",
  "async fn ensure_builder_publish_enabled_in_tx(",
  "Page Builder reviewed compilation owner",
);
requireOrder(
  compiler,
  [
    "reviewed.validate()",
    "reviewed.preview_runtime()",
    "sanitize_static_landing_project",
    "sanitized.verify_integrity()",
    "compile_materialized_static_landing",
    "materialized.verify_integrity()",
    "materialization_hash",
    "materialization_identity",
    "runtime_snapshots",
  ],
  "sanitization materialization ordering",
);

const publicArtifact = sliceBetween(
  artifactService,
  "pub async fn load_public_bound_artifact_with_fallback(",
  "async fn page_is_visible_for_channel_in_tx(",
  "public artifact owner read",
);
requireOrder(
  publicArtifact,
  [
    "page::Entity::find_by_id(page_id)",
    'page.status == "published"',
    "page_is_visible_for_channel_in_tx",
    "build_locale_candidates",
    "load_bound_artifact_in_tx",
    "txn.commit().await?",
  ],
  "public artifact channel and locale ordering",
);
const boundArtifact = sliceBetween(
  artifactService,
  "async fn load_bound_artifact_in_tx(",
  "async fn find_artifact_in_tx(",
  "bound immutable artifact read",
);
requireOrder(
  boundArtifact,
  [
    "page_body::Entity::find()",
    "CONTENT_FORMAT_GRAPESJS",
    "page_published_landing_artifact::Entity::find_by_id(body.id)",
    "page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)",
    "published_record(record).map(Some)",
  ],
  "binding before verification ordering",
);
requireText(
  artifactService,
  "verify_record(&record)?;",
  "published artifact integrity verification",
);
requireText(
  artifactService,
  "PageBuilderMaterializedStaticLandingArtifact",
  "full materialization envelope reconstruction",
);

for (const marker of [
  "Reviewed publication fixture",
  "Visible channel and immutable artifact URL",
  "Hidden-channel isolation",
  "Integrity failure cannot fill a fresh key",
  "A cache value cannot be created from an unverified immutable artifact",
  "execution list is empty",
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
  requireText(continuation, marker, "current parity continuation plan");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageCacheInvalidationPort",
  "publish_non_builder",
]) {
  forbidText(harness, forbidden, "reviewed artifact harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-reviewed-artifact] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-reviewed-artifact] PASS source_ready=true execution=pending",
);
