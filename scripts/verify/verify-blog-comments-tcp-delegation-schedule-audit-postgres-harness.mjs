import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const testPath = 'apps/server/tests/blog_comments_schedule_audit_postgres.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-84.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-postgres-harness.json';

const test = read(testPath);
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
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs':
    '8a27f5ec3938f2b4efe16c6acafb93cb3faadcf6',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs':
    'acb6c37a2f2c841dc5bffaf4cca74a97342731ae',
  'crates/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs':
    '305f4f80abcdd6da62d11f8c21eb8ab5101bd002',
  'apps/server/Cargo.toml': 'd6a77ff30fd67c3d4fee16afbadc584c2bcce9e7',
};
for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(test, [
  '#[ignore = "requires PostgreSQL admin access"]',
  'audited_schedule_success_resume_and_conflicts_are_atomic',
  'concurrent_audited_schedule_cas_commits_one_state_and_one_outbox',
  'PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore::new',
  'SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_host_document',
  'CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty',
  'CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact',
  'Migrator::up(&db_a, None).await?',
  'RUSTOK_MIGRATION_SMOKE_ADMIN_URL',
  'unique_postgres_database_name(prefix)',
  'trigger.replace_host_schedule(',
  'state_after_success.digest_hex != persisted.schedule_digest().to_hex()',
  'reused durable request identity unexpectedly committed',
  'seed_generation_conflict',
  'generation conflict did not roll back the attempted outbox insert',
  'Arc::new(Barrier::new(2))',
  'tokio::task::spawn_blocking',
  'local_generations != [1, 2]',
  'durable_request != winning_request',
  'CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceConflict',
  'tokio::time::sleep(Duration::from_millis(200)).await;',
], 'PostgreSQL integration harness');

requireCondition(
  (test.match(/#\[ignore = "requires PostgreSQL admin access"\]/g) ?? []).length === 2,
  'both PostgreSQL scenarios must remain ignored',
);
requireCondition(
  !test.includes('MockComments') && !test.includes('FakeComments'),
  'harness must not replace the production store with a mock',
);
requireCondition(
  !test.includes('CommentsTcpDelegationSchedulePersistenceRecord {'),
  'harness must not construct arbitrary persistence records',
);
requireCondition(
  !test.includes('compare_and_store_with_audit('),
  'harness must enter audited CAS through the public trigger',
);

hasAll(plan, [
  'public audited trigger and the full workspace migrator',
  'Request identity conflict rollback',
  'Candidate generation conflict rollback',
  'Concurrent CAS scenario',
  'does **not** inject a PostgreSQL commit acknowledgement failure',
  'Status: `source_verified_no_compile`',
  'intentionally not run',
], 'slice 84 plan');

requireCondition(evidence.status === 'source_verified_no_compile', 'evidence status drift');
requireCondition(evidence.harness.ignored_tests === true, 'ignored harness evidence missing');
requireCondition(evidence.harness.full_workspace_migrator === true, 'workspace migrator evidence missing');
requireCondition(evidence.harness.public_production_api_only === true, 'production API evidence missing');
requireCondition(evidence.atomic_success_resume.exact_resume_generation_two === true, 'exact resume evidence missing');
requireCondition(evidence.request_conflict_rollback.durable_state_remains_generation_two === true, 'request rollback evidence missing');
requireCondition(evidence.generation_conflict_rollback.state_update_rolled_back === true, 'generation rollback evidence missing');
requireCondition(evidence.concurrent_cas.expected_successes === 1, 'concurrent winner evidence drift');
requireCondition(evidence.concurrent_cas.expected_conflicts === 1, 'concurrent conflict evidence drift');
requireCondition(evidence.reconciliation_boundary.commit_acknowledgement_failure_injected === false, 'commit fault overclaim');
requireCondition(evidence.reconciliation_boundary.ambiguous_commit_runtime_claim === false, 'ambiguous commit overclaim');
requireCondition(evidence.execution.rust_tests_run === false, 'Rust execution overclaim');
requireCondition(evidence.execution.javascript_verifiers_run === false, 'verifier execution overclaim');
requireCondition(evidence.execution.postgresql_run === false, 'PostgreSQL execution overclaim');
requireCondition(evidence.execution.cargo_commands_run === false, 'Cargo execution overclaim');

console.log('Blog Comments audited PostgreSQL harness source contract verified');
