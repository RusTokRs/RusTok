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
    "crates/rustok-pages/contracts/evidence/pages-outbox-relay-restart-postgres-source.json",
  ),
);
const harness = read("crates/rustok-pages/tests/outbox_relay_restart_postgres.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const transactionalBus = read("crates/rustok-outbox/src/transactional.rs");
const cacheOwner = read("crates/rustok-pages/src/cache_invalidation.rs");
const overlay = read(
  "docs/modules/pages-page-builder-outbox-relay-restart-packet-2026-08-04.md",
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

if (evidence.status !== "pages_outbox_relay_restart_postgres_source_unvalidated") {
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
  "outbox_retry_observed",
  "relay_restart_observed",
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
  transactional_event_bus_used: true,
  outbox_transport_used: true,
  durable_node_published_committed_before_relay: true,
  first_relay_worker_has_distinct_identity: true,
  first_delivery_fails_before_cache_handler: true,
  failed_delivery_remains_pending: true,
  failed_delivery_increments_retry_count: true,
  failed_delivery_clears_claim: true,
  failed_delivery_does_not_set_dispatched_at: true,
  failed_delivery_does_not_rotate_cache_generations: true,
  second_relay_instance_has_distinct_worker_identity: true,
  second_relay_reclaims_pending_row: true,
  second_delivery_uses_same_event_id: true,
  second_delivery_uses_same_root_correlation_id: true,
  second_delivery_drives_real_pages_cache_handler: true,
  handler_request_binds_event_id: true,
  handler_request_binds_correlation_id: true,
  handler_receipt_binds_event_id: true,
  handler_receipt_binds_correlation_id: true,
  route_page_artifact_generations_rotate_once: true,
  successful_delivery_marks_dispatched: true,
  successful_delivery_sets_dispatched_at: true,
  successful_delivery_clears_retry_error_and_claim: true,
  acknowledgement_occurs_after_target_publish: true,
  production_outbox_behavior_changed: false,
  production_pages_cache_behavior_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  storefront_http_executed: false,
  artifact_http_executed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !== "crates/rustok-pages/tests/outbox_relay_restart_postgres.rs" ||
  evidence.harness?.test !==
    "restarted_relay_dispatches_pending_node_published_before_acknowledging_row" ||
  evidence.harness?.database_env !== "RUSTOK_PAGES_TEST_DATABASE_URL" ||
  evidence.harness?.fallback_database_env !== "DATABASE_URL" ||
  evidence.harness?.first_worker_id !== "pages-relay-before-restart" ||
  evidence.harness?.second_worker_id !== "pages-relay-after-restart"
) {
  failures.push("relay restart harness registration is invalid");
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  "struct TestDatabase",
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "OutboxModule.migrations()",
  "TransactionalEventBus::new(outbox_transport)",
  "publish_in_tx_with_envelope_id(",
  "txn.commit().await?",
  "struct RestartTarget",
  "simulated Pages cache target outage",
  "PageCacheInvalidationEventHandler::new",
  "first_relay.process_pending_once(Some(1)).await?",
  "restarted_relay.process_pending_once(Some(1)).await?",
  'relay_config("pages-relay-before-restart")',
  'relay_config("pages-relay-after-restart")',
  "assert_eq!(retrying.status, SysEventStatus::Pending)",
  "assert_eq!(retrying.retry_count, 1)",
  "assert!(retrying.dispatched_at.is_none())",
  "assert_eq!(dispatched.status, SysEventStatus::Dispatched)",
  "assert!(dispatched.dispatched_at.is_some())",
  "assert_eq!(target.delivered_event_ids(), vec![event_id])",
  "assert_eq!(generations, PageCacheGenerationSnapshot::new(1, 1, 1))",
  "assert_eq!(requests[0].event_id, event_id)",
  "assert_eq!(requests[0].correlation_id, event_id)",
  "assert_eq!(receipts[0].event_id, event_id)",
  "assert_eq!(receipts[0].correlation_id, event_id)",
]) {
  requireText(harness, marker, "relay restart PostgreSQL harness");
}

const testBody = sliceBetween(
  harness,
  "async fn restarted_relay_dispatches_pending_node_published_before_acknowledging_row(",
  "fn relay_config(",
  "relay restart test",
);
requireOrder(
  testBody,
  [
    "publish_in_tx_with_envelope_id(",
    "txn.commit().await?",
    "assert_eq!(pending.status, SysEventStatus::Pending)",
    "first_relay.process_pending_once(Some(1)).await?",
    "assert_eq!(retrying.status, SysEventStatus::Pending)",
    "assert!(retrying.dispatched_at.is_none())",
    "restarted_relay.process_pending_once(Some(1)).await?",
    "assert_eq!(dispatched.status, SysEventStatus::Dispatched)",
    "assert!(dispatched.dispatched_at.is_some())",
    "assert_eq!(generations, PageCacheGenerationSnapshot::new(1, 1, 1))",
  ],
  "durable retry restart acknowledgement ordering",
);

const relayProcess = sliceBetween(
  relay,
  "async fn process_claimed_event(&self, model: &entity::Model)",
  "fn decode_envelope(model: &entity::Model)",
  "outbox relay claimed-event processing",
);
requireOrder(
  relayProcess,
  [
    "self.target.publish(envelope).await",
    "self.mark_dispatched(model).await?",
    "self.record_processed(elapsed_ms, true)",
  ],
  "target delivery before durable acknowledgement",
);
requireOrder(
  relayProcess,
  [
    "Err(err) =>",
    "self.record_processed(elapsed_ms, false)",
    "self.mark_failed_attempt(model, err).await",
  ],
  "delivery failure retry ordering",
);

const relayFailure = sliceBetween(
  relay,
  "async fn mark_failed_attempt(&self, model: &entity::Model, error: Error)",
  "fn backoff_duration(&self, retry_count: i32)",
  "outbox relay failed-attempt persistence",
);
for (const marker of [
  "retry_count: Set(retry_count)",
  "last_error: Set(Some(error.to_string()))",
  "claimed_by: Set(None)",
  "claimed_at: Set(None)",
  "status: Set(status)",
  "next_attempt_at: Set(next_attempt_at)",
]) {
  requireText(relayFailure, marker, "failed-attempt retry state");
}

const relayAck = sliceBetween(
  relay,
  "async fn mark_dispatched(&self, model: &entity::Model)",
  "async fn mark_failed_attempt(&self, model: &entity::Model, error: Error)",
  "outbox relay acknowledgement persistence",
);
for (const marker of [
  "status: Set(SysEventStatus::Dispatched)",
  "dispatched_at: Set(Some(Utc::now()))",
  "claimed_by: Set(None)",
  "claimed_at: Set(None)",
  "last_error: Set(None)",
  "next_attempt_at: Set(None)",
]) {
  requireText(relayAck, marker, "successful acknowledgement state");
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
  "receipt.validate_for(&request)?",
  "DomainEvent::NodePublished",
  "Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES",
]) {
  requireText(cacheOwner, marker, "Pages cache handler contract");
}
for (const forbidden of ["CacheService", "redis::", 'cmd("SCAN")', 'cmd("KEYS")']) {
  forbidText(harness, forbidden, "restart harness ownership boundary");
}

for (const marker of [
  "Outbox relay restart packet: ready, unvalidated",
  "first relay worker",
  "second relay instance",
  "acknowledgement is persisted only after target delivery succeeds",
  "PostgreSQL execution remains pending",
  "artifact HTTP packet remains open",
]) {
  requireText(overlay, marker, "relay restart continuation overlay");
}

if (failures.length > 0) {
  console.error("[verify-pages-outbox-relay-restart-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-outbox-relay-restart-postgres] PASS source_ready=true postgres_execution=pending",
);
