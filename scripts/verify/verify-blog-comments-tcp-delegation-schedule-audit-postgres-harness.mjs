import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { flattenWhitespace } from './lib/verify-text.mjs';

// This guard used to require that the files of the slice stayed byte-identical to a recorded git blob
// SHA. A digest describes one commit, not an invariant: the pin fired on every legitimate edit of files
// this guard does not own (rustok-events, apps/server/Cargo.toml, Cargo.lock), while every behavioural
// assertion below was intact. The snapshot is preserved as a record in
// crates/modules/rustok-blog/docs/comments-delegation-preserved-owner-snapshot-2026-10-26.md; the live
// rules are the markers, which are unchanged.

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');

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

hasAllProse(plan, [
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
