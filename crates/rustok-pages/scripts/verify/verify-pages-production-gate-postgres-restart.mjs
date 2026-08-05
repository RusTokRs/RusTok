#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-production-gate-postgres-restart-source.json",
));
const harness = read("apps/server/tests/pages_production_gate_postgres_restart.rs");
const gate = read("apps/server/src/services/tenant_generation_delivery_gate.rs");
const port = read("apps/server/src/services/pages_cache_invalidation.rs");
const relay = read("crates/rustok-outbox/src/relay.rs");
const historicalOwner = read("crates/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs");
const historicalRestart = read("crates/rustok-pages/tests/outbox_relay_restart_postgres.rs");
const packet = read("docs/modules/pages-page-builder-production-gate-postgres-restart-packet-2026-08-05.md");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let at = -1;
  for (const marker of markers) {
    at = text.indexOf(marker, at + 1);
    if (at < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_production_gate_postgres_restart_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_production_gate_postgres_restart_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "server_level_postgres_harness_added",
  "isolated_postgres_schema_per_run",
  "real_outbox_module_migrations_used",
  "real_pages_module_migrations_used",
  "transactional_event_bus_used",
  "durable_publish_receipt_and_event_committed",
  "durable_rollback_receipt_and_event_committed",
  "real_outbox_relay_used",
  "production_tenant_generation_delivery_gate_used",
  "production_server_pages_cache_port_used",
  "publish_rotation_precedes_outbox_acknowledgement",
  "publish_route_page_artifact_generations_rotate_once",
  "publish_new_keys_miss_then_refill",
  "publish_old_generation_values_remain_physically_present",
  "rollback_downstream_failure_occurs_after_generation_rotation",
  "failed_rollback_delivery_remains_pending",
  "failed_rollback_delivery_increments_retry_count",
  "failed_rollback_delivery_does_not_set_dispatched_at",
  "first_relay_worker_has_distinct_identity",
  "second_relay_instance_has_distinct_identity",
  "second_relay_reclaims_same_pending_event",
  "second_delivery_uses_same_event_and_correlation_identity",
  "process_bounded_dedupe_prevents_second_rotation",
  "successful_retry_marks_outbox_dispatched",
  "successful_retry_clears_error_and_claim",
  "ordinary_pages_listener_same_event_is_rotation_noop",
  "rollback_new_keys_miss_then_refill",
  "rollback_old_generation_values_remain_physically_present",
  "historical_postgres_owner_packet_remains_separate",
  "historical_restart_pre_handler_failure_packet_remains_separate"
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_outbox_behavior_changed",
  "production_cache_policy_changed",
  "database_schema_changed",
  "public_route_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}
if (
  evidence.harness?.path !== "apps/server/tests/pages_production_gate_postgres_restart.rs" ||
  evidence.harness?.test !== "production_gate_correlates_postgres_publish_rollback_and_restart_retry" ||
  evidence.harness?.database_env !== "RUSTOK_PAGES_TEST_DATABASE_URL" ||
  evidence.harness?.fallback_database_env !== "DATABASE_URL" ||
  evidence.harness?.publish_worker_id !== "pages-production-gate-publish" ||
  evidence.harness?.first_rollback_worker_id !== "pages-production-gate-before-restart" ||
  evidence.harness?.second_rollback_worker_id !== "pages-production-gate-after-restart"
) {
  failures.push("production gate PostgreSQL harness registration is invalid");
}

for (const marker of [
  '#![cfg(feature = "mod-pages")]',
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "OutboxModule",
  "PagesModule",
  "TransactionalEventBus::new(outbox_transport)",
  "CacheService::from_url(None)",
  "start_tenant_cache_generation_listener",
  "ServerPagesCachePort::new(&cache)",
  "TenantGenerationDeliveryGate::new",
  "OutboxRelay::new",
  "production_gate_correlates_postgres_publish_rollback_and_restart_retry",
  "persist_publish_receipt_and_event",
  "persist_rollback_receipt_and_event",
  "synthetic downstream rejection after Pages generation rotation",
  'relay_config("pages-production-gate-publish")',
  'relay_config("pages-production-gate-before-restart")',
  'relay_config("pages-production-gate-after-restart")',
  "PageCacheGenerationSnapshot::new(1, 1, 1)",
  "PageCacheGenerationSnapshot::new(2, 2, 2)",
  "assert_retrying(&db, rollback_event_id)",
  "assert_dispatched(&db, rollback_event_id, 1)",
  "PageCacheInvalidationEventHandler::new",
  ".handle(&delivered_rollback)",
  "assert_new_keys_miss_and_old_keys_remain",
  "refill_new_keys",
  "assert_old_and_new_values"
]) need(harness, marker, "server PostgreSQL harness");

const testBody = between(
  harness,
  "async fn production_gate_correlates_postgres_publish_rollback_and_restart_retry(",
  "async fn seed_old_keys(",
  "production gate PostgreSQL test",
);
ordered(testBody, [
  "persist_publish_receipt_and_event(",
  "seed_old_keys(",
  "TenantGenerationDeliveryGate::new(",
  'relay_config("pages-production-gate-publish")',
  "publish_relay.process_pending_once(Some(1)).await?",
  "assert_dispatched(&db, publish_event_id, 0).await?",
  "PageCacheGenerationSnapshot::new(1, 1, 1)",
  "assert_new_keys_miss_and_old_keys_remain",
  "refill_new_keys",
  "persist_rollback_receipt_and_event(",
  "downstream.fail_next()",
  'relay_config("pages-production-gate-before-restart")',
  "first_rollback_relay.process_pending_once(Some(1)).await?",
  "assert_retrying(&db, rollback_event_id).await?",
  "PageCacheGenerationSnapshot::new(2, 2, 2)",
  'relay_config("pages-production-gate-after-restart")',
  "restarted_relay.process_pending_once(Some(1)).await?",
  "assert_dispatched(&db, rollback_event_id, 1).await?",
  ".handle(&delivered_rollback)",
  "refill_new_keys(&reads, &rollback_keys, \"rollback\")",
  "assert_old_and_new_values"
], "publish rollback restart ordering");

ordered(gate, [
  "self.ensure_local_listener_ready().await?",
  "self.pages_handler.handles(&envelope.event)",
  "self.pages_handler.handle(&envelope).await?",
  "self.inner.publish(envelope).await"
], "production delivery gate ordering");
ordered(port, [
  "serialize_event(request.event_id)",
  "is_duplicate(request.event_id)",
  "self.generations.bump(&namespace).await",
  "receipt.validate_for(&request)?",
  "self.successful_invalidations.observe(request.event_id)"
], "production Pages dedupe ordering");
need(port, "OnceLock<Arc<BoundedCacheEventDedupe>>", "production Pages dedupe");

const relayProcess = between(
  relay,
  "async fn process_claimed_event(&self, model: &entity::Model)",
  "fn decode_envelope(model: &entity::Model)",
  "outbox relay process",
);
ordered(relayProcess, [
  "self.target.publish(envelope).await",
  "Ok(()) =>",
  "self.mark_dispatched(model).await?"
], "outbox success acknowledgement ordering");
ordered(relayProcess, [
  "Err(err) =>",
  "self.record_processed(elapsed_ms, false)",
  "self.mark_failed_attempt(model, err).await"
], "outbox failure retry ordering");

for (const marker of [
  "PageCacheInvalidationEventHandler::new",
  "handler.handle(envelope).await?",
  "publish_and_rollback_receipts_correlate_with_durable_outbox_and_cache_rotation_on_postgres"
]) need(historicalOwner, marker, "historical owner PostgreSQL harness");
forbid(historicalOwner, "TenantGenerationDeliveryGate::new", "historical owner PostgreSQL harness");
for (const marker of [
  "struct RestartTarget",
  "simulated Pages cache target outage",
  "restarted_relay_dispatches_pending_node_published_before_acknowledging_row"
]) need(historicalRestart, marker, "historical restart PostgreSQL harness");
forbid(historicalRestart, "TenantGenerationDeliveryGate::new", "historical restart PostgreSQL harness");

for (const marker of [
  "source-ready / execution-pending",
  "post-invalidation downstream failure",
  "process-bounded dedupe prevents a second rotation",
  "historical PostgreSQL publish/rollback packet remains authoritative",
  "Execution evidence remains pending"
]) need(packet, marker, "production gate PostgreSQL packet");
for (const marker of [
  "production-gate-postgres-restart-source-ready",
  "Production gate PostgreSQL publish/rollback restart: source-ready",
  "post-invalidation downstream failure",
  "historical owner-transaction and pre-handler restart packets remain separate"
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "production gate PostgreSQL publish/rollback restart harness",
  "post-invalidation downstream failure",
  "process-bounded dedupe prevents a",
  "second rotation when a new relay instance retries"
]) need(localPlan, marker, "Pages local plan");

if (failures.length) {
  console.error("[verify-pages-production-gate-postgres-restart] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-production-gate-postgres-restart] PASS source_ready=true postgres_execution=pending");
