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
    "crates/rustok-pages/contracts/evidence/pages-publish-rollback-outbox-cache-postgres-source.json",
  ),
);
const harness = read(
  "crates/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs",
);
const reviewedPublish = read(
  "crates/rustok-pages/src/services/page/reviewed_publish.rs",
);
const rollback = read("crates/rustok-pages/src/services/page/rollback.rs");
const transactionalBus = read("crates/rustok-outbox/src/transactional.rs");
const outboxTransport = read("crates/rustok-outbox/src/transport.rs");
const outboxEntity = read("crates/rustok-outbox/src/entity.rs");
const cacheOwner = read("crates/rustok-pages/src/cache_invalidation.rs");
const overlay = read(
  "docs/modules/pages-page-builder-postgres-outbox-cache-packet-2026-08-04.md",
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
  "pages_publish_rollback_outbox_cache_postgres_source_unvalidated"
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
  "postgres_run",
  "outbox_rows_observed",
  "cache_handler_run",
  "storefront_run",
  "artifact_http_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  postgres_environment_gated_harness_added: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  transactional_event_bus_used: true,
  outbox_transport_used: true,
  publish_page_version_advanced_in_owner_transaction: true,
  publish_node_published_written_before_receipt: true,
  publish_receipt_written_before_commit: true,
  rollback_page_version_advanced_in_owner_transaction: true,
  rollback_node_published_written_before_receipt: true,
  rollback_receipt_written_before_commit: true,
  durable_envelope_loaded_from_sys_events: true,
  durable_envelope_registered_schema_validated: true,
  root_correlation_id_equals_event_id: true,
  receipt_insert_conflict_rolls_back_prior_outbox_insert: true,
  durable_publish_envelope_drives_cache_handler: true,
  durable_rollback_envelope_drives_cache_handler: true,
  handler_request_binds_event_id: true,
  handler_request_binds_correlation_id: true,
  handler_receipt_binds_event_id: true,
  handler_receipt_binds_correlation_id: true,
  route_page_artifact_generations_rotate_per_event: true,
  storefront_current_key_misses_then_refills: true,
  artifact_current_key_misses_then_refills: true,
  old_generation_values_remain_unreachable_by_current_keys: true,
  production_publish_behavior_changed: false,
  production_rollback_behavior_changed: false,
  production_cache_behavior_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
    "crates/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs" ||
  evidence.harness?.test !==
    "publish_and_rollback_receipts_correlate_with_durable_outbox_and_cache_rotation_on_postgres" ||
  evidence.harness?.database_env !== "RUSTOK_PAGES_TEST_DATABASE_URL" ||
  evidence.harness?.fallback_database_env !== "DATABASE_URL"
) {
  failures.push("PostgreSQL harness registration is invalid");
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  "struct TestDatabase",
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "OutboxModule",
  "PagesModule",
  "OutboxTransport::new(db.clone())",
  "TransactionalEventBus::new(transport)",
  "publish_and_rollback_receipts_correlate_with_durable_outbox_and_cache_rotation_on_postgres",
  "SysEvents::find_by_id(event_id)",
  "envelope.validate_registered_schema()?",
  "assert_eq!(envelope.correlation_id, event_id)",
  "PageCacheInvalidationEventHandler::new",
  "input.handler.handle(input.envelope).await?",
  "put_json(new_storefront_key.clone(), &refilled_storefront)",
  "put_json(new_artifact_key.clone(), &refilled_artifact)",
  "assert_event_absent(&db, rolled_back_event_id).await?",
]) {
  requireText(harness, marker, "PostgreSQL harness");
}

const publishFixture = sliceBetween(
  harness,
  "async fn persist_publish_receipt_and_event(",
  "async fn persist_conflicting_publish_and_rollback(",
  "publish PostgreSQL fixture",
);
requireOrder(
  publishFixture,
  [
    "let txn = db.begin().await?;",
    "UPDATE pages SET status = 'published'",
    ".publish_in_tx_with_envelope_id(",
    "INSERT INTO page_publish_operations",
    "txn.commit().await?;",
  ],
  "publish page event receipt commit ordering",
);

const conflictFixture = sliceBetween(
  harness,
  "async fn persist_conflicting_publish_and_rollback(",
  "async fn persist_rollback_receipt_and_event(",
  "publish receipt conflict fixture",
);
requireOrder(
  conflictFixture,
  [
    "let txn = db.begin().await?;",
    ".publish_in_tx_with_envelope_id(",
    "INSERT INTO page_publish_operations",
    "assert!(duplicate.is_err());",
    "txn.rollback().await?;",
  ],
  "outbox rollback after receipt conflict",
);

const rollbackFixture = sliceBetween(
  harness,
  "async fn persist_rollback_receipt_and_event(",
  "async fn read_published_envelope(",
  "rollback PostgreSQL fixture",
);
requireOrder(
  rollbackFixture,
  [
    "let txn = db.begin().await?;",
    "UPDATE pages SET updated_at = CURRENT_TIMESTAMP, version = 3",
    ".publish_in_tx_with_envelope_id(",
    "INSERT INTO page_rollback_operations",
    "txn.commit().await?;",
  ],
  "rollback page event receipt commit ordering",
);

const cacheCycle = sliceBetween(
  harness,
  "async fn rotate_and_refill(",
  "async fn read_publish_receipt_version(",
  "durable cache cycle",
);
requireOrder(
  cacheCycle,
  [
    "let before = input.reads.generation_snapshot(input.tenant_id).await?;",
    "input.handler.handle(input.envelope).await?;",
    "let after = input.reads.generation_snapshot(input.tenant_id).await?;",
    "get_json::<Value>(&new_storefront_key)",
    "get_json::<Value>(&new_artifact_key)",
    "put_json(new_storefront_key.clone(), &refilled_storefront)",
    "put_json(new_artifact_key.clone(), &refilled_artifact)",
    "get_json::<Value>(&old_storefront_key)",
    "get_json::<Value>(&old_artifact_key)",
  ],
  "durable generation miss refill ordering",
);

const publishOwner = sliceBetween(
  reviewedPublish,
  "pub async fn publish_reviewed(",
  "fn require_builder_sources(",
  "production reviewed publish owner",
);
requireOrder(
  publishOwner,
  [
    "let txn = self.db.begin().await?;",
    "DomainEvent::NodePublished",
    "insert_publish_operation_in_tx(",
    "txn.commit().await?;",
  ],
  "production publish event receipt commit ordering",
);
for (const forbidden of ["CacheService", "PagesCacheReadRuntime", "page_cache_namespace"]) {
  forbidText(publishOwner, forbidden, "production publish inline cache boundary");
}

const rollbackOwner = sliceBetween(
  rollback,
  "pub async fn rollback_to_previous(",
  "async fn find_previous_publish_target_in_tx(",
  "production rollback owner",
);
requireOrder(
  rollbackOwner,
  [
    "let txn = self.db.begin().await?;",
    "DomainEvent::NodePublished",
    "insert_rollback_operation_in_tx(",
    "txn.commit().await?;",
  ],
  "production rollback event receipt commit ordering",
);
for (const forbidden of ["CacheService", "PagesCacheReadRuntime", "page_cache_namespace"]) {
  forbidText(rollbackOwner, forbidden, "production rollback inline cache boundary");
}

for (const marker of [
  "pub async fn publish_in_tx_with_envelope_id",
  "let envelope_id = envelope.id;",
  "outbox.write_to_outbox(txn, envelope).await?;",
  "Ok(envelope_id)",
]) {
  requireText(transactionalBus, marker, "transactional event bus");
}
for (const marker of [
  "pub async fn write_to_outbox",
  "Self::write_envelope_in_tx(txn, envelope).await",
  "entity::Entity::insert(Self::model_from_envelope(envelope)?)",
]) {
  requireText(outboxTransport, marker, "outbox transport");
}
for (const marker of [
  '#[sea_orm(table_name = "sys_events")]',
  "pub payload: Json",
  "pub status: SysEventStatus",
]) {
  requireText(outboxEntity, marker, "durable outbox entity");
}
for (const marker of [
  "receipt.validate_for(&request)?",
  "DomainEvent::NodePublished",
  "Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES",
]) {
  requireText(cacheOwner, marker, "Pages cache owner");
}

for (const marker of [
  "PostgreSQL receipt/outbox/cache packet: ready, unvalidated",
  "`OutboxModule` and `PagesModule`",
  "receipt-conflict transaction writes the outbox envelope first",
  "PostgreSQL execution remains pending",
]) {
  requireText(overlay, marker, "PostgreSQL continuation overlay");
}

if (failures.length > 0) {
  console.error("[verify-pages-publish-rollback-outbox-cache-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-publish-rollback-outbox-cache-postgres] PASS source_ready=true postgres_execution=pending",
);
