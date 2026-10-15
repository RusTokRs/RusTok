import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const testPath = 'apps/server/tests/blog_comments_schedule_audit_postgres.rs';
const planPath = 'crates/modules/rustok-blog/docs/implementation-plan-slice-84.md';
const evidencePath =
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-postgres-harness.json';

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
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs':
    '4094539df6158d0a899e641322ae84bdbb6a129f',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs':
    '8660fd1b96997bcdbaf52718847ce7c8b7eeb7e0',
  'crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs':
    '2f6473d7889a242ef86621b8615834e8bffe1013',
  'apps/server/Cargo.toml': 'e80ef5a046f33d493848e695164fa5954ec27d4d',
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
