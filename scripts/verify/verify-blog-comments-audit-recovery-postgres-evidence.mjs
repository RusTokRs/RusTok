#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const runtimePath =
  'apps/server/src/services/comments_provider_runtime.rs';
const harnessPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_operator_postgres_evidence.rs';
const operatorPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_operator.rs';
const recoveryPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_recovery_postgres.rs';
const workerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_handoff_worker_source_retry.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-95.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-audit-recovery-postgres-evidence.json';

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

const runtime = read(runtimePath);
const harness = read(harnessPath);
const operator = read(operatorPath);
const recovery = read(recoveryPath);
const worker = read(workerPath);
const plan = read(planPath);
const evidenceText = read(evidencePath);
const evidence = JSON.parse(evidenceText);

requireAll('runtime mount', runtime, [
  'mod keyring_schedule_audit_operator {',
  'include!("comments_provider_runtime_keyring_schedule_audit_operator.rs");',
  'include!("comments_provider_runtime_keyring_schedule_audit_operator_postgres_evidence.rs");',
]);

requireAll('PostgreSQL evidence harness', harness, [
  '#[cfg(all(test, feature = "mod-blog"))]',
  'mod retained_postgres_evidence',
  'RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL',
  'std::env::var("DATABASE_URL")',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  'SET search_path TO',
  'rustok_blog::BlogModule.migrations()',
  'm20260801_000007_create_blog_comments_delegation_schedule_state',
  'm20260801_000008_create_blog_comments_delegation_schedule_audit_outbox',
  'm20260803_000009_add_blog_comments_audit_canonical_handoff',
  'm20260803_000010_add_blog_comments_audit_source_retry_policy',
  'm20260803_000011_create_blog_comments_audit_recovery',
  'authorization_precedes_validation_and_storage',
  'exact_inspection_requeue_and_append_only_audit_are_atomic',
  'stale_and_non_terminal_requeue_fail_closed',
  'concurrent_requeue_admits_one_epoch_and_next_worker_claim_starts_at_one',
  'CommentsTcpDelegationScheduleAuditOperatorError::MissingRequestAuthority',
  'CommentsTcpDelegationScheduleAuditOperatorError::Forbidden',
  'CommentsTcpDelegationScheduleAuditOperatorError::TenantMismatch',
  'CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable',
  'CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(_)',
  'CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection',
  'CommentsTcpDelegationScheduleAuditRecoveryOutcome::NotDeadLetter',
  'CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued',
  'attempt_budget_exhausted',
  'handoff_attempt_count',
  'handoff_recovery_epoch',
  'UPDATE {RECOVERY_AUDIT_TABLE}',
  'DELETE FROM {RECOVERY_AUDIT_TABLE}',
  'claim_next_retry_ready(8)',
  'claim.attempt_count() == 1',
  'recovery_audit_count(&context.db).await? == 1',
]);

const ignoredCount = (harness.match(/#\[ignore = "requires maintainer PostgreSQL execution"\]/g) ?? []).length;
if (ignoredCount !== 4) {
  throw new Error(`expected four ignored PostgreSQL scenarios, found ${ignoredCount}`);
}

forbidAll('PostgreSQL evidence harness', harness, [
  'ServerRuntimeContext',
  'start_comments_tcp_listener_if_enabled',
  'start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled',
  'RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
  'sys_events',
  'axum::',
  'async_graphql',
  'Mcp',
]);

requireAll('existing operator boundary', operator, [
  'context.authorize_for(self.control_plane_tenant_id)?;',
  'pub async fn inspect_dead_letter(',
  'pub async fn requeue_dead_letter(',
  'Permission::MODULES_MANAGE',
]);
requireAll('existing recovery owner', recovery, [
  'FOR UPDATE',
  'handoff_attempt_count = 0',
  'handoff_recovery_epoch = $4',
  'insert_recovery_audit_statement',
  'reconcile_requeue',
]);
requireAll('existing retry-aware worker', worker, [
  'claim_next_retry_ready(source_max_attempts)',
  'run_source_retry_handoff_cycle(',
]);

requireAll('slice 95 plan', plan, [
  'Status: `recovery_postgres_evidence_source_ready_maintainer_execution_pending`.',
  'Authorization before validation and storage',
  'Exact inspection and atomic audited requeue',
  'Closed stale and non-terminal outcomes',
  'Concurrent single epoch and worker admission',
  '--features mod-blog',
  '--ignored --nocapture --test-threads=1',
  'were not executed by the implementation agent',
  'restart and ambiguous-commit evidence',
]);

if (evidence.schema_version !== 1) {
  throw new Error('evidence schema_version must be 1');
}
if (
  evidence.status !==
  'recovery_postgres_evidence_source_ready_maintainer_execution_pending'
) {
  throw new Error(`unexpected evidence status: ${evidence.status}`);
}
if (!evidence.harness.cfg_test_only || !evidence.harness.unique_schema_per_scenario) {
  throw new Error('evidence must retain a test-only isolated PostgreSQL harness');
}
if (!evidence.authorization_ordering.missing_authority_before_storage) {
  throw new Error('missing-authority-before-storage evidence marker is absent');
}
if (!evidence.atomic_requeue.audit_update_rejected || !evidence.atomic_requeue.audit_delete_rejected) {
  throw new Error('append-only recovery audit evidence markers are incomplete');
}
if (!evidence.concurrency.exactly_one_requeued || !evidence.worker_admission.first_attempt_is_one) {
  throw new Error('concurrency or retry-aware worker admission evidence is incomplete');
}
for (const [name, value] of Object.entries(evidence.validation)) {
  if (value !== false) {
    throw new Error(`validation.${name} must remain false until maintainer execution`);
  }
}
for (const [name, value] of Object.entries(evidence.transport)) {
  if (value !== false) {
    throw new Error(`transport.${name} must remain false`);
  }
}
for (const [name, value] of Object.entries(evidence.preserved_contracts)) {
  if (value !== false) {
    throw new Error(`preserved_contracts.${name} must remain false`);
  }
}

console.log('Blog Comments audit recovery PostgreSQL evidence source guard passed');
