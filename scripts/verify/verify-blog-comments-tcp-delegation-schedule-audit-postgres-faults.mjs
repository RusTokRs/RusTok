import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { flattenWhitespace } from './lib/verify-text.mjs';

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
  'crates/modules/rustok-blog/docs/implementation-plan-slice-85.md';
const evidencePath =
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-postgres-faults.json';

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
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs':
    '4094539df6158d0a899e641322ae84bdbb6a129f',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs':
    '8660fd1b96997bcdbaf52718847ce7c8b7eeb7e0',
  'crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs':
    '2f6473d7889a242ef86621b8615834e8bffe1013',
  'apps/server/tests/blog_comments_schedule_audit_postgres.rs':
    '7b726fe69de5d678021bb376ea4ca9a0cb2b89e6',
  'apps/server/Cargo.toml':
    'f72bc1ce6f615150c86afa55ae0bc5b3478b366b',
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

hasAllProse(plan, [
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
