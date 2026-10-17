import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readComposedRustSource } from './lib/rust-facade-source.mjs';
import { flattenWhitespace } from './lib/verify-text.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const guardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger_guard.rs';
const auditedStorePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs';
const auditedTriggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs';
const migrationRegistryPath = 'crates/modules/rustok-blog/src/migrations/mod.rs';
const migrationPath =
  'crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs';
const planPath = 'crates/modules/rustok-blog/docs/implementation-plan-slice-83.md';
const evidencePath =
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-outbox.json';

const runtime = readComposedRustSource(read, runtimePath);
const guard = readComposedRustSource(read, guardPath);
const auditedStore = readComposedRustSource(read, auditedStorePath);
const auditedTrigger = readComposedRustSource(read, auditedTriggerPath);
const migrationRegistry = read(migrationRegistryPath);
const migration = read(migrationPath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}

function hasAll(source, markers, label) {
  for (const marker of markers) {
    requireCondition(source.includes(marker), `${label} missing marker: ${marker}`);
  }
}

function hasAllProse(content, markers, label) {
  // See scripts/verify/lib/verify-text.mjs: prose markers are wording contracts, so they
  // are compared against a whitespace-flattened view of the source.
  const flattened = flattenWhitespace(content);
  for (const marker of markers) {
    requireCondition(
      flattened.includes(flattenWhitespace(marker)),
      `${label} missing marker: ${marker}`,
    );
  }
}

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

const preserved = {
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs':
    'ffcb6586e3dc3f1b2d6f21f4d1ab405fb5657fcd',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs':
    '4a65f209a7fc23d769160166b3b8bbe462916115',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs':
    'f805a7c0c733f4a17ed79c4d019f32b90a7e606d',
  'crates/modules/rustok-comments/src/tcp_delegation_schedule.rs':
    '2cd2a8c2bba97bff61ea5a37fd21405c2c482717',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs':
    '5834d1021dfad4fed23e8ade98aff816f004e865',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs':
    '17a3be2732cd7fbd48981ff417265055ce86e4b2',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs':
    '9ec8190ff19ad1603e60a5d60c566bb8f417ef6c',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres.rs':
    '8f3a5d8b988ad278ebbe33d00f7c663448f6f62f',
};
for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(runtime, [
  'mod keyring_schedule_persistence_postgres_audit',
  'mod keyring_schedule_postgres_audited_trigger',
  'PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore',
  'SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger',
], 'runtime facade');

hasAll(guard, [
  'postgres_audited_trigger',
  'configured_trigger_count > 1',
  'PostgreSQL-audited, persisted, and process-local',
  'standalone schedule handle cannot be combined',
], 'trigger guard');

hasAll(auditedTrigger, [
  'struct PostgresAuditedPersistenceBridge',
  'pending: Mutex<',
  'let _operation = self.0.operation.lock()',
  'let _pending = self.0.bridge.install(audit_context)?;',
  'AuditedExecutionCompletionGuard { completed: false }',
  'completion.completed = true;',
  'compare_and_store_with_audit(expected, candidate, &audit)',
  'abort_on_indeterminate_audited_store_response()',
], 'audited trigger');

hasAll(auditedStore, [
  'blog_comments_tcp_delegation_schedule_audit_outbox',
  'comments_tcp_delegation_schedule_replaced',
  'CompareAndStoreWithAudit',
  'execute_raw(update_state_statement(&expected, &candidate))',
  'execute_raw(insert_audit_statement(&audit))',
  'if state_rows != 1',
  'if audit_rows != 1',
  'transaction.commit().await',
  'read_current_record(database).await',
  'read_audit_record(database, audit.request_id).await',
  'current == candidate && &current_audit == audit',
  'abort_on_indeterminate_postgres_audit_commit()',
  'ON CONFLICT DO NOTHING',
], 'audited PostgreSQL store');

const stateUpdate = auditedStore.indexOf(
  'execute_raw(update_state_statement(&expected, &candidate))',
);
const auditInsert = auditedStore.indexOf('execute_raw(insert_audit_statement(&audit))');
const commit = auditedStore.indexOf('transaction.commit().await', auditInsert);
requireCondition(
  stateUpdate >= 0 && stateUpdate < auditInsert && auditInsert < commit,
  'state / outbox / commit ordering drift',
);

hasAll(migrationRegistry, [
  'mod m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox;',
  'm20260801_000008_create_blog_comments_delegation_schedule_audit_outbox::Migration',
], 'migration registry');

hasAll(migration, [
  'blog_comments_tcp_delegation_schedule_audit_outbox',
  'RequestId',
  'AuditSchemaVersion',
  'OccurredAtUnixMs',
  'ActorId',
  'PrincipalKind',
  'PreviousGeneration',
  'CandidateGeneration',
  'PublishedAt',
  'uq_blog_comments_delegation_audit_generation',
  'ForeignKeyAction::Restrict',
  'intentionally irreversible',
], 'audit outbox migration');

hasAllProse(plan, [
  'same PostgreSQL transaction as the exact state CAS',
  'Only exact candidate state plus exact audit row is accepted as success',
  'does not claim durable completeness',
  'Status: `source_verified_no_compile`',
], 'slice 83 plan');

requireCondition(evidence.status === 'source_verified_no_compile', 'evidence status drift');
requireCondition(evidence.transaction.audit_insert_same_transaction === true, 'atomic outbox evidence missing');
requireCondition(evidence.commit_reconciliation.exact_candidate_and_exact_audit_is_success === true, 'pair reconciliation evidence missing');
requireCondition(evidence.audit_scope.durable_audit_completeness_claim === false, 'durable completeness overclaim');
requireCondition(evidence.outbox_delivery.publisher === false, 'publisher overclaim');
requireCondition(evidence.execution.rust_tests_run === false, 'Rust execution overclaim');
requireCondition(evidence.execution.javascript_verifiers_run === false, 'verifier execution overclaim');
requireCondition(evidence.execution.postgresql_run === false, 'PostgreSQL execution overclaim');

console.log('Blog Comments delegation schedule audit-outbox source contract verified');
