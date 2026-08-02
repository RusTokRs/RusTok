import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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
const migrationRegistryPath = 'crates/rustok-blog/src/migrations/mod.rs';
const migrationPath =
  'crates/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-83.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-outbox.json';

const runtime = read(runtimePath);
const guard = read(guardPath);
const auditedStore = read(auditedStorePath);
const auditedTrigger = read(auditedTriggerPath);
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

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

const preserved = {
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs':
    '7d0c48df3dcea5a77bebe99dcdd4f02292f7ef47',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs':
    'af2d688d43286a8118823c31408dd5dc7e3f6202',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs':
    'f44f386c36ac81d51da89ee5568ffd9af083cda0',
  'crates/rustok-comments/src/tcp_delegation_schedule.rs':
    '7701953d2892e1b68b4830135639d841ebdd6ed1',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs':
    '85df8deb3203ff90be55a8fe114294ce3a1f3749',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs':
    'f3b37405f06e3cc4bd077cf8bd49f92d9e2cbe7a',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs':
    'a90bdd8e4787532bd39c1739dfef8b6a18eaa981',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres.rs':
    '0a6399cbf0a2dcbd5390475111e25a88a8d03361',
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
  'execute(update_state_statement(&expected, &candidate))',
  'execute(insert_audit_statement(&audit))',
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
  'execute(update_state_statement(&expected, &candidate))',
);
const auditInsert = auditedStore.indexOf('execute(insert_audit_statement(&audit))');
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

hasAll(plan, [
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
