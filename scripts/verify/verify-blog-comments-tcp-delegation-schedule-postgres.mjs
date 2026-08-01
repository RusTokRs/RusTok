import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const adapterPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres.rs';
const persistencePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs';
const persistedTriggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs';
const bridgePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs';
const scheduleBasePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs';
const scheduleGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs';
const processTriggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs';
const commentsSchedulePath = 'crates/rustok-comments/src/tcp_delegation_schedule.rs';
const migrationModPath = 'crates/rustok-blog/src/migrations/mod.rs';
const migrationPath =
  'crates/rustok-blog/src/migrations/m20260801_000007_create_blog_comments_delegation_schedule_state.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-82.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-postgres.json';
const serverManifestPath = 'apps/server/Cargo.toml';
const blogManifestPath = 'crates/rustok-blog/Cargo.toml';
const lockPath = 'Cargo.lock';

const runtime = read(runtimePath);
const adapter = read(adapterPath);
const migrationMod = read(migrationModPath);
const migration = read(migrationPath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

function requireCondition(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function hasAll(source, markers, label) {
  for (const marker of markers) {
    requireCondition(source.includes(marker), `${label} missing marker: ${marker}`);
  }
}

function hasNone(source, markers, label) {
  for (const marker of markers) {
    requireCondition(!source.includes(marker), `${label} forbidden marker: ${marker}`);
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
  [scheduleBasePath]: '7d0c48df3dcea5a77bebe99dcdd4f02292f7ef47',
  [scheduleGuardPath]: 'af2d688d43286a8118823c31408dd5dc7e3f6202',
  [processTriggerPath]: 'f44f386c36ac81d51da89ee5568ffd9af083cda0',
  [commentsSchedulePath]: '7701953d2892e1b68b4830135639d841ebdd6ed1',
  [persistencePath]: '85df8deb3203ff90be55a8fe114294ce3a1f3749',
  [persistedTriggerPath]: 'f3b37405f06e3cc4bd077cf8bd49f92d9e2cbe7a',
  [bridgePath]: 'a90bdd8e4787532bd39c1739dfef8b6a18eaa981',
  [serverManifestPath]: 'd6a77ff30fd67c3d4fee16afbadc584c2bcce9e7',
  [blogManifestPath]: 'd71db5d5f7e43ed969485ea9ab508f11b9554902',
  [lockPath]: 'c862fa6acc5f77642c1ce860e8810ca4cfc1a3c2',
};
for (const [relativePath, expected] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expected,
    `preserved source drift: ${relativePath}`,
  );
}

hasAll(
  runtime,
  [
    'include!("comments_provider_runtime_keyring_schedule_persistence_postgres.rs");',
    'COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY',
    'COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE',
    'PostgresCommentsTcpDelegationSchedulePersistenceStore',
  ],
  'runtime PostgreSQL persistence export',
);

hasAll(
  adapter,
  [
    'pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY: &str',
    '"comments_tcp_delegation_schedule"',
    'pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE: &str',
    '"blog_comments_tcp_delegation_schedule_state"',
    'pub struct PostgresCommentsTcpDelegationSchedulePersistenceStore',
    'database.get_database_backend() != DbBackend::Postgres',
    'mpsc::sync_channel(POSTGRES_STORE_QUEUE_CAPACITY)',
    'Builder::new_current_thread().enable_all().build()',
    'impl persistence::CommentsTcpDelegationSchedulePersistenceStore',
    'pub fn into_shared(',
    'database.begin().await',
    'ON CONFLICT (state_key) DO NOTHING',
    'AND schema_version = $6',
    'AND source = $7',
    'AND generation = $8',
    'AND schedule_digest_hex = $9',
    'transaction.commit().await',
    'reconcile_ambiguous_commit(',
    'COMMIT_RECONCILIATION_ATTEMPTS: usize = 20',
    'COMMIT_RECONCILIATION_DELAY_MS: u64 = 100',
    'std::process::abort()',
  ],
  'PostgreSQL persistence adapter',
);

const beginIndex = adapter.indexOf('let transaction = database.begin().await');
const executeIndex = adapter.indexOf('let execution = match expected.as_ref()');
const rowsIndex = adapter.indexOf('let rows_affected = match execution');
const commitIndex = adapter.indexOf('match transaction.commit().await');
const reconcileIndex = adapter.indexOf('reconcile_ambiguous_commit(', commitIndex);
requireCondition(
  beginIndex >= 0
    && beginIndex < executeIndex
    && executeIndex < rowsIndex
    && rowsIndex < commitIndex
    && commitIndex < reconcileIndex,
  'PostgreSQL transaction / CAS / commit ordering drift',
);

const candidateReadIndex = adapter.indexOf('if &current == candidate');
const expectedReadIndex = adapter.indexOf('if current.as_ref() == expected');
const abortIndex = adapter.indexOf('abort_on_indeterminate_postgres_commit()', expectedReadIndex);
requireCondition(
  candidateReadIndex >= 0
    && candidateReadIndex < expectedReadIndex
    && expectedReadIndex < abortIndex,
  'ambiguous commit reconciliation ordering drift',
);

hasNone(
  adapter,
  [
    'println!(',
    'eprintln!(',
    'tracing::',
    'secret',
    'key_id',
    'file_path',
    'DATABASE_URL',
    'std::env::',
    'tokio::spawn',
  ],
  'PostgreSQL adapter secret and ambient configuration boundary',
);

hasAll(
  migrationMod,
  [
    'mod m20260801_000007_create_blog_comments_delegation_schedule_state;',
    'm20260801_000007_create_blog_comments_delegation_schedule_state::Migration',
  ],
  'Blog migration registry',
);

hasAll(
  migration,
  [
    'blog_comments_tcp_delegation_schedule_state',
    '.string_len(64)',
    '.small_integer()',
    '.string_len(16)',
    '.big_integer()',
    'ScheduleDigestHex',
    '.timestamp_with_time_zone()',
    'schema_version = 1',
    "source IN ('host_provided', 'file')",
    'generation > 0',
    'length(schedule_digest_hex) = 64',
  ],
  'Blog PostgreSQL persistence migration',
);

hasNone(
  migration,
  ['Secret', 'KeyId', 'Credential', 'Nonce', 'Token', 'FilePath', 'ActorId', 'RequestId'],
  'persistence table secret and identity columns',
);

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.table?.name === 'blog_comments_tcp_delegation_schedule_state'
    && evidence.table?.state_key === 'comments_tcp_delegation_schedule'
    && evidence.table?.singleton_primary_key === true
    && evidence.table?.secret_values === false
    && evidence.table?.key_ids === false,
  'table evidence drift',
);
requireCondition(
  evidence.adapter?.database_backend === 'postgresql_only'
    && evidence.adapter?.implements_slice_81_store_trait === true
    && evidence.adapter?.accepts_connection_url === false
    && evidence.adapter?.new_direct_dependencies?.length === 0,
  'adapter evidence drift',
);
requireCondition(
  evidence.commit?.ambiguous_commit_reconciled === true
    && evidence.commit?.candidate_readback_is_success === true
    && evidence.commit?.expected_readback_is_unavailable === true
    && evidence.commit?.third_state_fail_stop === true
    && evidence.commit?.unreadable_after_retries_fail_stop === true
    && evidence.commit?.indeterminate_error_returned === false,
  'commit reconciliation evidence drift',
);
requireCondition(
  evidence.execution?.rust_tests_run === false
    && evidence.execution?.javascript_verifiers_run === false
    && evidence.execution?.cargo_commands_run === false
    && evidence.execution?.formatting_run === false
    && evidence.execution?.postgresql_run === false
    && evidence.execution?.workflow_run === false
    && evidence.execution?.ci_run === false,
  'execution evidence drift',
);

hasAll(
  plan,
  [
    '## Slice 82 — PostgreSQL schedule persistence adapter',
    '`PostgresCommentsTcpDelegationSchedulePersistenceStore`',
    '`INSERT ... ON CONFLICT (state_key) DO NOTHING`',
    'The complete expected record participates in the predicate',
    'Ambiguous commit reconciliation',
    'fail-stop the process',
    '`std::process::abort()`',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-82 plan',
);

console.log(
  'Blog Comments TCP delegation schedule PostgreSQL persistence source verification passed',
);
