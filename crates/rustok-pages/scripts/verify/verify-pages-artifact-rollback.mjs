#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const dto = read("crates/rustok-pages/src/dto/page.rs");
const errors = read("crates/rustok-pages/src/error.rs");
const rollback = read("crates/rustok-pages/src/services/page/rollback.rs");
const artifactSet = read("crates/rustok-pages/src/services/page/artifact_set.rs");
const manifestWriter = read("crates/rustok-pages/src/services/page/publish_manifest.rs");
const publishEntity = read("crates/rustok-pages/src/entities/page_publish_operation.rs");
const manifestEntity = read(
  "crates/rustok-pages/src/entities/page_publish_operation_artifact.rs",
);
const rollbackEntity = read(
  "crates/rustok-pages/src/entities/page_rollback_operation.rs",
);
const migration = read(
  "crates/rustok-pages/src/migrations/m20260722_000009_create_page_rollback_operations.rs",
);
const migrations = read("crates/rustok-pages/src/migrations/mod.rs");
const graphqlTypes = read("crates/rustok-pages/src/graphql/types.rs");
const graphqlMutation = read("crates/rustok-pages/src/graphql/mutation.rs");
const http = read("crates/rustok-pages/src/http.rs");
const openapi = read("crates/rustok-pages/src/openapi.rs");
const adminModel = read("crates/rustok-pages/admin/src/model.rs");
const adminTransport = read(
  "crates/rustok-pages/admin/src/transport/graphql_adapter.rs",
);
const adminRollbackRetry = read(
  "crates/rustok-pages/admin/src/transport/rollback_retry_adapter.rs",
);
const adminTransportModule = read(
  "crates/rustok-pages/admin/src/transport/mod.rs",
);
const adminRollbackControl = read(
  "crates/rustok-pages/admin/src/rollback_control.rs",
);
const adminLib = read("crates/rustok-pages/admin/src/lib.rs");

function fail(message) {
  console.error(`[verify-pages-artifact-rollback] ${message}`);
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

for (const marker of [
  "pub struct RollbackPageInput",
  "pub expected_version: i32",
  "pub idempotency_key: String",
  "pub struct RollbackPageResult",
  "pub target_publish_operation_id: Uuid",
  "pub source_artifact_set_hash: String",
  "pub target_artifact_set_hash: String",
  "pub replayed: bool",
]) {
  requireMarker(dto, marker, "rollback DTO contract");
}

for (const marker of [
  "RollbackIdempotencyConflict(String)",
  "RollbackOperationIntegrity(String)",
  "RollbackTargetUnavailable(String)",
  "RollbackRequiresPublished",
  "PAGE_ROLLBACK_IDEMPOTENCY_CONFLICT",
  "PAGE_ROLLBACK_OPERATION_INTEGRITY",
  "PAGE_ROLLBACK_TARGET_UNAVAILABLE",
  "PAGE_ROLLBACK_REQUIRES_PUBLISHED",
]) {
  requireMarker(errors, marker, "typed rollback errors");
}

for (const marker of [
  'table_name = "page_publish_operation_artifacts"',
  "pub operation_id: Uuid",
  "pub locale: String",
  "pub artifact_id: Uuid",
  "pub artifact_hash: String",
  "pub materialization_hash: Option<String>",
]) {
  requireMarker(manifestEntity, marker, "immutable publish manifest entity");
}
for (const marker of [
  'table_name = "page_rollback_operations"',
  "pub idempotency_key: String",
  "pub request_hash: String",
  "pub target_publish_operation_id: Uuid",
  "pub source_artifact_set_hash: String",
  "pub target_artifact_set_hash: String",
  "pub result_version: i32",
]) {
  requireMarker(rollbackEntity, marker, "rollback receipt entity");
}
for (const marker of [
  "PagePublishOperationArtifacts::OperationId",
  "PagePublishOperationArtifacts::Locale",
  ".unique()",
  "PageRollbackOperations::TenantId",
  "PageRollbackOperations::PageId",
  "PageRollbackOperations::IdempotencyKey",
  "fk_page_rollback_operations_target_publish",
]) {
  requireMarker(migration, marker, "rollback migration");
}
requireMarker(
  migrations,
  "m20260722_000009_create_page_rollback_operations",
  "rollback migration registration",
);

for (const marker of [
  "impl ActiveModelBehavior for ActiveModel",
  "async fn after_save<C>(model: Model, db: &C, insert: bool)",
  "persist_publish_manifest_after_save(db, &model)",
]) {
  requireMarker(publishEntity, marker, "publish receipt manifest invariant");
}
for (const marker of [
  "pub(crate) async fn persist_publish_manifest_after_save",
  "page_published_landing_artifact::Entity::find()",
  "page_static_landing_artifact::Entity::find_by_id",
  "manifest_hash != operation.artifact_set_hash",
  "page_publish_operation_artifact::ActiveModel",
]) {
  requireMarker(manifestWriter, marker, "publish manifest writer");
}

for (const marker of [
  "pub(super) fn artifact_set_hash",
  "load_publish_manifest_in_tx",
  "load_current_published_set_in_tx",
  "replace_current_published_set_in_tx",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "page_published_landing_artifact::Entity::delete_many()",
]) {
  requireMarker(artifactSet, marker, "artifact set owner");
}

for (const marker of [
  "pub async fn rollback_to_previous",
  "find_rollback_operation_in_tx",
  "rollback_result_from_record(operation, true)",
  'existing_page.status != "published"',
  "load_current_published_set_in_tx",
  "find_previous_publish_target_in_tx",
  "let mut current_index = None",
  "load_publish_manifest_in_tx(txn, operation).await?",
  ".skip(current_index + 1)",
  "replace_current_published_set_in_tx",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "insert_rollback_operation_in_tx",
  "txn.commit().await?",
]) {
  requireMarker(rollback, marker, "atomic rollback service");
}
requireOrderedMarkers(
  rollback,
  [
    "find_page_for_update",
    "find_rollback_operation_in_tx",
    "enforce_expected_version",
    "load_current_published_set_in_tx",
    "find_previous_publish_target_in_tx",
    "replace_current_published_set_in_tx",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "insert_rollback_operation_in_tx",
    "txn.commit().await?",
  ],
  "atomic rollback operation order",
);
for (const forbidden of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "PageBuilderPreviewRuntime",
  "PageBuilderReviewedPublishRuntime",
  "CacheService",
  "PagesCacheInvalidationRuntime",
]) {
  forbidMarker(rollback, forbidden, "rollback must reuse immutable artifacts only");
}

for (const marker of [
  "pub struct RollbackGqlPageInput",
  "pub struct GqlRollbackPageResult",
  "impl From<crate::RollbackPageResult> for GqlRollbackPageResult",
]) {
  requireMarker(graphqlTypes, marker, "GraphQL rollback types");
}
for (const marker of [
  "async fn rollback_page",
  "input: RollbackGqlPageInput",
  "Result<GqlRollbackPageResult>",
  ".rollback_to_previous(",
]) {
  requireMarker(graphqlMutation, marker, "GraphQL rollback mutation");
}
for (const marker of [
  'path = "/api/admin/pages/{id}/rollback"',
  "request_body = RollbackPageInput",
  "HttpResult<Json<RollbackPageResult>>",
  ".rollback_to_previous(",
  '"/api/admin/pages/{id}/rollback"',
]) {
  requireMarker(http, marker, "HTTP rollback route");
}
for (const marker of [
  "crate::http::rollback_page",
  "crate::RollbackPageInput",
  "crate::RollbackPageResult",
]) {
  requireMarker(openapi, marker, "rollback OpenAPI surface");
}

for (const marker of [
  "pub struct RollbackPageReceipt",
  "RolledBack(RollbackPageReceipt)",
]) {
  requireMarker(adminModel, marker, "admin rollback model");
}
for (const marker of [
  "$input: RollbackGqlPageInput!",
  "rollbackPage(id: $id, input: $input)",
  "ROLLBACK_IDEMPOTENCY_FORMAT",
  "pub async fn rollback_page",
  "RollbackPageReceipt",
]) {
  requireMarker(adminTransport, marker, "admin non-browser rollback GraphQL transport");
}
for (const marker of [
  "ROLLBACK_RETRY_STORAGE_PREFIX",
  "struct PendingRollbackAttempt",
  "load_pending_attempt(&page_id)?",
  "store_pending_attempt(&page_id, &attempt)?",
  "attempt.expected_version",
  "attempt.idempotency_key.clone()",
  "is_definitive_rejection(&error)",
  "GraphqlHttpError::Network => false",
  ".session_storage()",
]) {
  requireMarker(adminRollbackRetry, marker, "admin browser rollback retry identity");
}
for (const marker of [
  '#[cfg(target_arch = "wasm32")]\nmod rollback_retry_adapter;',
  "rollback_retry_adapter::rollback_page(token, tenant_slug, id).await?",
  "graphql_adapter::rollback_page(token, tenant_slug, id).await?",
  "PagePublicationResult::RolledBack(receipt)",
  "validate_publication_result",
]) {
  requireMarker(adminTransportModule, marker, "admin rollback transport facade");
}

for (const marker of [
  "pub(crate) fn PagesRollbackControl",
  "use_route_query_value(AdminQueryKey::PageId.as_str())",
  'page.status.eq_ignore_ascii_case("published")',
  "transport::rollback_page(token, tenant, page_id)",
  "result.version()",
  "on_rolled_back.run(())",
  '"Rollback"',
]) {
  requireMarker(adminRollbackControl, marker, "Pages admin rollback control");
}
for (const marker of [
  "mod rollback_control;",
  "use rollback_control::PagesRollbackControl;",
  "pub fn PagesAdmin()",
  "<PagesRollbackControl on_rolled_back />",
  "<PagesWorkspace />",
]) {
  requireMarker(adminLib, marker, "Pages admin rollback boundary composition");
}

console.log("[verify-pages-artifact-rollback] PASS");
