#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const handoffSupportPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres_test_support.rs';
const recoverySupportPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_recovery_postgres_test_support.rs';
const harnessPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_restart_ambiguity_postgres_evidence.rs';
const handoffPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs';
const recoveryPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_recovery_postgres.rs';
const writerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_canonical_writer.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-96.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-audit-restart-ambiguity-evidence.json';

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function requireAll(label, text, markers) {
  for (const marker of markers) {
    if (!text.includes(marker)) {
      throw new Error(`${label} is missing required marker: ${marker}`);
    }
  }
}

function forbidAll(label, text, markers) {
  for (const marker of markers) {
    if (text.includes(marker)) {
      throw new Error(`${label} contains forbidden marker: ${marker}`);
    }
  }
}

for (const file of [
  runtimePath,
  handoffSupportPath,
  recoverySupportPath,
  harnessPath,
  handoffPath,
  recoveryPath,
  writerPath,
  planPath,
  evidencePath,
]) {
  if (!fs.existsSync(path.join(root, file))) {
    throw new Error(`required slice-96 file is missing: ${file}`);
  }
}

const runtime = read(runtimePath);
const handoffSupport = read(handoffSupportPath);
const recoverySupport = read(recoverySupportPath);
const harness = read(harnessPath);
const handoff = read(handoffPath);
const recovery = read(recoveryPath);
const writer = read(writerPath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

requireAll('runtime test mounts', runtime, [
  'include!("comments_provider_runtime_keyring_schedule_audit_recovery_postgres_test_support.rs");',
  'include!("comments_provider_runtime_keyring_schedule_audit_restart_ambiguity_postgres_evidence.rs");',
  'include!("comments_provider_runtime_keyring_schedule_audit_operator_postgres_evidence.rs");',
]);

requireAll('handoff test seam', handoffSupport, [
  '#[cfg(test)]',
  'impl PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff',
  'pub(crate) async fn reconcile_claim_for_test',
  'self.reconcile_claim(claim_token).await',
  'pub(crate) async fn reconcile_publication_for_test',
  'self.reconcile_publication(request_id).await',
]);
requireAll('recovery test seam', recoverySupport, [
  '#[cfg(test)]',
  'impl PostgresCommentsTcpDelegationScheduleAuditRecoveryStore',
  'pub(crate) async fn reconcile_requeue_for_test',
  'self.reconcile_requeue(audit_id, request, recovery_epoch)',
]);
forbidAll('test seams', handoffSupport + recoverySupport, [
  'Statement::',
  'execute_unprepared',
  'tokio::spawn',
  'OutboxRelay',
]);

requireAll('existing handoff reconciliation', handoff, [
  'async fn reconcile_claim(',
  'read_claim_statement(claim_token)',
  'async fn reconcile_publication(',
  'read_publication_statement(request_id)',
  'published && canonical_envelope_id == Some(request_id)',
]);
requireAll('existing recovery reconciliation', recovery, [
  'async fn reconcile_requeue(',
  'reconcile_requeue_statement(audit_id)',
  'stored_tenant != request.control_plane_tenant_id',
  'stored_request != request.request_id',
  'stored_actor != request.actor_id',
  'stored_reason != request.reason',
  'stored_prior_attempt != request.expected_attempt_count',
  'stored_epoch != recovery_epoch',
]);
requireAll('production canonical writer', writer, [
  'RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
  'publish_contract_once_direct_in_tx_with_envelope_id',
  'delivery remains owned by',
]);

requireAll('restart ambiguity harness', harness, [
  '#[cfg(all(test, feature = "mod-blog"))]',
  'mod retained_restart_ambiguity_evidence',
  'RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL',
  'rustok_outbox::OutboxModule.migrations()',
  'rustok_blog::BlogModule.migrations()',
  'm20260801_000007_create_blog_comments_delegation_schedule_state',
  'm20260801_000008_create_blog_comments_delegation_schedule_audit_outbox',
  'm20260803_000009_add_blog_comments_audit_canonical_handoff',
  'm20260803_000010_add_blog_comments_audit_source_retry_policy',
  'm20260803_000011_create_blog_comments_audit_recovery',
  'active_claim_ack_reconciles_after_owner_restart',
  'expired_claim_is_reclaimed_after_restart_and_old_token_is_fenced',
  'publication_ack_reconciles_after_restart_without_running_relay',
  'requeue_ack_reconciles_exact_audit_facts_after_restart',
  'claim_next_retry_ready(8)',
  'reconcile_claim_for_test',
  'reconcile_publication_for_test',
  'reconcile_requeue_for_test',
  'RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
  'FROM sys_events WHERE id',
  'canonical.status == "pending"',
  'canonical.retry_count == 0',
  'CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState',
  'DROP SCHEMA IF EXISTS',
  'SET search_path TO',
]);
const ignoredCount =
  (harness.match(/#\[ignore = "requires maintainer PostgreSQL execution"\]/g) ?? [])
    .length;
if (ignoredCount !== 4) {
  throw new Error(`expected four ignored restart/ambiguity scenarios, found ${ignoredCount}`);
}
forbidAll('restart ambiguity harness ownership', harness, [
  'OutboxRelay::',
  'RelayConfig',
  'start_comments_tcp_listener_if_enabled',
  'start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled',
  'ServerRuntimeContext',
  'axum::',
  'async_graphql',
]);

requireAll('slice 96 plan', plan, [
  'Status: `restart_ambiguity_postgres_evidence_source_ready_maintainer_execution_pending`.',
  'Active claim acknowledgement',
  'Expired claim restart recovery',
  'Publication acknowledgement',
  'Requeue acknowledgement',
  'does not inject a database-driver failure into production code',
  'No relay task is started',
  '--ignored --nocapture --test-threads=1',
  'were not executed by the implementation agent',
]);

if (evidence.schema_version !== 1) {
  throw new Error('evidence schema_version must be 1');
}
if (
  evidence.status !==
  'restart_ambiguity_postgres_evidence_source_ready_maintainer_execution_pending'
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}
const requiredTrue = [
  [evidence.harness?.cfg_test_only, 'harness.cfg_test_only'],
  [evidence.harness?.new_connection_after_commit, 'harness.new_connection_after_commit'],
  [evidence.test_seams?.handoff_reconcile_claim_wrapper, 'test_seams.claim'],
  [evidence.test_seams?.handoff_reconcile_publication_wrapper, 'test_seams.publication'],
  [evidence.test_seams?.recovery_reconcile_requeue_wrapper, 'test_seams.requeue'],
  [evidence.claim_ack?.exact_claim_token_reconciled, 'claim_ack.token'],
  [evidence.expired_claim_restart?.old_token_fenced, 'expired_claim_restart.old_token'],
  [evidence.publication_ack?.production_canonical_writer_used, 'publication_ack.writer'],
  [evidence.publication_ack?.exactly_one_sys_event, 'publication_ack.cardinality'],
  [evidence.requeue_ack?.mismatched_reason_rejected, 'requeue_ack.mismatch'],
  [evidence.requeue_ack?.exactly_one_recovery_audit, 'requeue_ack.cardinality'],
];
for (const [value, label] of requiredTrue) {
  if (value !== true) {
    throw new Error(`${label} must be true`);
  }
}
if (evidence.publication_ack?.relay_started !== false) {
  throw new Error('publication_ack.relay_started must remain false');
}
for (const [name, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) {
    throw new Error(`validation.${name} must remain false until maintainer execution`);
  }
}
for (const [name, value] of Object.entries(evidence.transport ?? {})) {
  if (value !== false) {
    throw new Error(`transport.${name} must remain false`);
  }
}
for (const [name, value] of Object.entries(evidence.preserved_contracts ?? {})) {
  if (value !== false) {
    throw new Error(`preserved_contracts.${name} must remain false`);
  }
}

console.log('Blog Comments audit restart/ambiguity evidence source guard passed');
