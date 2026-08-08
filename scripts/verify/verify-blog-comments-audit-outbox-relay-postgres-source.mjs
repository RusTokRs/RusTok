#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const files = {
  evidence: "crates/rustok-outbox/contracts/evidence/blog-comments-audit-relay-postgres-source.json",
  test: "crates/rustok-outbox/tests/blog_comments_audit_relay_postgres.rs",
  relay: "crates/rustok-outbox/src/relay.rs",
  transactional: "crates/rustok-outbox/src/transactional.rs",
  event: "crates/rustok-events/src/blog_comments_schedule_audit.rs",
  writer: "apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_canonical_writer.rs",
  outboxPlan: "crates/rustok-outbox/docs/implementation-plan.md",
  previousPlan: "crates/rustok-blog/docs/implementation-plan-slice-96.md",
  plan: "crates/rustok-blog/docs/implementation-plan-slice-97.md",
};
const failures = [];
const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length) {
  console.error("[verify-blog-comments-audit-outbox-relay-postgres-source] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const evidence = JSON.parse(read(files.evidence));
const test = read(files.test);
const relay = read(files.relay);
const transactional = read(files.transactional);
const event = read(files.event);
const writer = read(files.writer);
const outboxPlan = read(files.outboxPlan);
const previousPlan = read(files.previousPlan);
const plan = read(files.plan);

if (
  evidence.schema_version !== 1 ||
  evidence.status !== "blog_comments_audit_outbox_relay_postgres_source_unvalidated" ||
  evidence.owner !== "rustok-outbox" ||
  evidence.consumer_contract !== "blog.comments_delegation_schedule.replacement_succeeded"
) failures.push("evidence identity/status/ownership drifted");

for (const key of [
  "postgres_environment_gated_harness_added",
  "isolated_postgres_schema_per_scenario",
  "real_outbox_module_migrations_used",
  "sealed_blog_comments_contract_used",
  "canonical_write_once_identity_used",
  "request_id_is_envelope_id",
  "request_id_is_correlation_id",
  "first_relay_worker_has_distinct_identity",
  "failed_delivery_remains_pending",
  "failed_delivery_increments_retry_count",
  "failed_delivery_clears_claim",
  "failed_delivery_does_not_set_dispatched_at",
  "second_relay_worker_has_distinct_identity",
  "restart_reclaims_same_durable_envelope",
  "successful_target_delivery_precedes_outbox_acknowledgement",
  "successful_delivery_marks_dispatched",
  "successful_delivery_clears_retry_error_and_claim",
  "successful_delivery_preserves_exact_event_identity",
  "successful_delivery_preserves_exact_tenant_and_actor",
  "attempt_budget_exhaustion_moves_to_failed_dlq",
  "dlq_preserves_exact_event_identity",
  "dlq_never_sets_dispatched_at",
  "dlq_clears_claim_and_retry_schedule",
]) if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);

for (const key of [
  "production_outbox_behavior_changed",
  "production_blog_behavior_changed",
  "database_schema_changed",
  "public_transport_changed",
  "canonical_relay_execution_observed",
  "postgres_execution_observed",
  "ffa_promoted",
  "fba_promoted",
]) if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);

if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence.execution must remain empty before maintainer execution");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}

for (const marker of [
  "RUSTOK_OUTBOX_BLOG_AUDIT_TEST_DATABASE_URL",
  "for migration in OutboxModule.migrations()",
  "BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded",
  "TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id",
  "blog_audit_relay_restarts_and_acknowledges_only_after_delivery",
  "blog_audit_relay_retries_then_moves_exact_envelope_to_dlq",
  'relay_config("blog-audit-relay-before-restart", 3)',
  'relay_config("blog-audit-relay-after-restart", 3)',
  'relay_config("blog-audit-dlq-before-restart", 2)',
  'relay_config("blog-audit-dlq-after-restart", 2)',
  "assert_eq!(retrying.status, SysEventStatus::Pending)",
  "assert_eq!(dispatched.status, SysEventStatus::Dispatched)",
  "assert_eq!(failed.status, SysEventStatus::Failed)",
  "assert_eq!(failed.id, request_id)",
  "assert!(failed.dispatched_at.is_none())",
  "ContractEventPayload::BlogCommentsDelegationScheduleAudit",
  "correlation_id: request_id",
]) need(test, marker, "PostgreSQL harness");
for (const marker of [
  "INSERT INTO sys_events",
  "UPDATE sys_events",
  "DELETE FROM sys_events",
  "tokio::spawn",
  "OutboxTransport::write_contract_envelope_once_in_tx",
]) forbid(test, marker, "PostgreSQL harness");

for (const marker of [
  "status: `restart_ambiguity_postgres_evidence_source_ready_maintainer_execution_pending`",
  "Retain canonical relay restart, delivery acknowledgement, retry, and DLQ evidence",
  "`rustok-outbox`-owned slice",
]) need(previousPlan, marker, "slice 96 cursor");

for (const marker of [
  "canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending",
  "rustok-outbox` remains the only relay/retry/DLQ owner",
  "Retry -> owner reconstruction -> delivery acknowledgement",
  "Retry -> owner reconstruction -> DLQ",
  "only after target success may the durable row become",
  "source rows and the immutable recovery-audit ledger",
  "No tests, Cargo commands, Node verifiers",
]) need(plan, marker, "slice 97 plan");

for (const marker of [
  "blog-comments-audit-relay-postgres-source.json",
  "retry, relay-owner reconstruction, delivery",
  "source-ready and unexecuted",
  "verify-blog-comments-audit-outbox-relay-postgres-source.mjs",
  "broader durable consumer-completion gap",
]) need(outboxPlan, marker, "Outbox implementation plan");

for (const marker of [
  "pub async fn process_pending_once",
  "SysEventStatus::Failed",
  "retry_count >= self.config.max_attempts",
  "self.mark_dispatched(model).await?",
  "self.mark_failed_attempt(model, err).await",
]) need(relay, marker, "Outbox relay owner");
for (const marker of [
  "publish_contract_once_direct_in_tx_with_envelope_id",
  "OutboxTransport::write_contract_envelope_once_in_tx",
]) need(transactional, marker, "Outbox transactional owner");
for (const marker of [
  "BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE",
  "BlogCommentsDelegationScheduleAuditEvent",
  "blog.comments_delegation_schedule.replacement_succeeded",
]) need(event, marker, "Blog audit sealed event contract");
for (const marker of [
  "RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter",
  "publication.idempotency_key()",
  "TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id",
  "Delivery remains owned by",
]) need(writer, marker, "Blog canonical writer");

if (failures.length) {
  console.error("[verify-blog-comments-audit-outbox-relay-postgres-source] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-blog-comments-audit-outbox-relay-postgres-source] PASS source_ready=true execution=not_run owner=rustok-outbox",
);
