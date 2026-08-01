import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) =>
  readFileSync(path.join(root, relativePath));

const testPath =
  'apps/server/tests/blog_comments_schedule_audit_postgres_faults.rs';
const planPath =
  'crates/rustok-blog/docs/implementation-plan-slice-85.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-postgres-faults.json';

const test = read(testPath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}

function hasAll(source, markers, label) {
  for (const marker of markers) {
    requireCondition(
      source.includes(marker),
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
  'apps/server/tests/blog_comments_schedule_audit_postgres.rs':
    '4ebe3a285495a8cbeb69729a9ca7f3452456785d',
  'apps/server/Cargo.toml':
    'd6a77ff30fd67c3d4fee16afbadc584c2bcce9e7',
};

for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(test, [
  'commit_ack_loss_exact_pair_reconciles_successfully',
  'commit_ack_loss_third_state_fail_stops',
  'commit_ack_loss_unreadable_reconciliation_fail_stops',
  'blog_comments_schedule_audit_fault_child',
  'std::env::current_exe()?',
  '--exact',
  '--ignored',
  '--test-threads=1',
  'kill_on_drop(true)',
  'CHILD_TIMEOUT',
  'ConnectOptions::new(database_url)',
  '.max_connections(1)',
  'CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact',
  'PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore::new',
  'SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_host_document',
  'TcpListener::bind(("127.0.0.1", 0))',
  'POSTGRES_SSL_REQUEST_CODE',
  'POSTGRES_GSSENC_REQUEST_CODE',
  'fault proxy requires plaintext PostgreSQL',
  "message_type == b'Z'",
  'frontend_query(message_type, &body)',
  'matches!(normalized.as_str(), "COMMIT" | "COMMIT TRANSACTION")',
  'inject_third_state(&shared.mutation_db)',
  'THIRD_STATE_DIGEST_HEX',
  'shared.fault_consumed.load(Ordering::Acquire)',
  'output.status.signal() != Some(6)',
  'RUSTOK_MIGRATION_SMOKE_ADMIN_URL',
  'Migrator::up(&database, None).await?',
], 'PostgreSQL commit fault harness');

requireCondition(
  (
    test.match(
      /#\[ignore = "requires PostgreSQL admin access and subprocess execution"\]/g,
    ) ?? []
  ).length === 3,
  'three parent fault scenarios must remain ignored',
);
requireCondition(
  test.includes(
    '#[ignore = "subprocess entry point for audited PostgreSQL fault harness"]',
  ),
  'child subprocess entry must remain ignored',
);
requireCondition(
  !test.includes('new_with_fault')
    && !test.includes('FaultInjectingPersistenceStore'),
  'harness must not add or depend on a production fault constructor',
);
requireCondition(
  !test.includes('compare_and_store_with_audit('),
  'child must enter audited CAS through the public trigger',
);
requireCondition(
  !test.includes('MockComments') && !test.includes('FakeComments'),
  'harness must not replace the production store with a mock',
);

hasAll(plan, [
  'drops the first audited replacement commit response',
  'PostgreSQL emits `ReadyForQuery`',
  'production store execute its real bounded reconciliation path',
  'Exact-pair recovery scenario',
  'Third-state fail-stop scenario',
  'Unreadable retry-exhaustion scenario',
  'Worker-response disconnect boundary',
  'Status: `source_verified_no_compile`',
  'intentionally not compiled or executed',
], 'slice 85 plan');

requireCondition(
  evidence.status === 'source_verified_no_compile',
  'evidence status drift',
);
requireCondition(
  evidence.harness.production_fault_constructor_added === false,
  'production fault constructor overclaim',
);
requireCondition(
  evidence.proxy.waits_for_ready_for_query === true,
  'ReadyForQuery boundary evidence missing',
);
requireCondition(
  evidence.proxy.withholds_commit_response === true,
  'commit response drop evidence missing',
);
requireCondition(
  evidence.exact_pair.production_reconciliation_path === true,
  'exact-pair reconciliation evidence missing',
);
requireCondition(
  evidence.third_state.expected_process_abort === true,
  'third-state fail-stop evidence missing',
);
requireCondition(
  evidence.unreadable_reconciliation.production_attempts === 20,
  'retry attempt evidence drift',
);
requireCondition(
  evidence.unreadable_reconciliation.production_delay_ms === 100,
  'retry delay evidence drift',
);
requireCondition(
  evidence.worker_response_disconnect.remains_open === true,
  'worker disconnect boundary must remain explicit',
);
requireCondition(
  evidence.execution.rust_tests_run === false,
  'Rust execution overclaim',
);
requireCondition(
  evidence.execution.javascript_verifiers_run === false,
  'verifier execution overclaim',
);
requireCondition(
  evidence.execution.postgresql_run === false,
  'PostgreSQL execution overclaim',
);
requireCondition(
  evidence.execution.proxy_run === false,
  'proxy execution overclaim',
);
requireCondition(
  evidence.execution.subprocess_run === false,
  'subprocess execution overclaim',
);
requireCondition(
  evidence.execution.signal_observed === false,
  'signal observation overclaim',
);

console.log(
  'Blog Comments audited PostgreSQL commit-fault harness source contract verified',
);
