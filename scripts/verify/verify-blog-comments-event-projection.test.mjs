#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve('scripts/verify/verify-blog-comments-event-projection.mjs');

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingTenantScope = false,
  missingPostLock = false,
  missingDeliveryOrdering = false,
  missingStateDelta = false,
  missingOutbox = false,
  missingRegistration = false,
  missingHostHarness = false,
  missingDispatcherCase = false,
  missingDispatcherWait = false,
  missingConcurrencyCase = false,
  missingConcurrencyBarrier = false,
  missingPostgresHarness = false,
  missingRollbackCase = false,
  missingPostgresRegistration = false,
  missingRestartHarness = false,
  missingRestartConnection = false,
  missingRestartRegistration = false,
  missingProcessRestartCase = false,
  missingProcessWorker = false,
  singleProcessWorkerInvocation = false,
  statusDrift = false,
  harnessStatusDrift = false,
  hostHarnessStatusDrift = false,
  dispatcherStatusDrift = false,
  concurrencyStatusDrift = false,
  postgresStatusDrift = false,
  restartStatusDrift = false,
  processRestartStatusDrift = false,
  publicSearchPathFallback = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-projection-'));
  const evidencePath = 'crates/modules/rustok-blog/contracts/evidence/blog-comments-event-projection.json';
  const handlerPath = 'crates/modules/rustok-blog/src/services/comment_projection.rs';
  const postgresHarnessPath = 'crates/modules/rustok-blog/tests/comment_projection_postgres_test.rs';
  const restartHarnessPath = 'crates/modules/rustok-blog/tests/comment_projection_restart_postgres_test.rs';
  const serviceExportPath = 'crates/modules/rustok-blog/src/services/mod.rs';
  const entityPath = 'crates/modules/rustok-blog/src/entities/blog_comment_projection_delivery.rs';
  const migrationPath = 'crates/modules/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs';
  const migrationRegistryPath = 'crates/modules/rustok-blog/src/migrations/mod.rs';
  const modulePath = 'crates/modules/rustok-blog/src/module.rs';
  const registryPath = 'crates/modules/rustok-blog/contracts/blog-fba-registry.json';
  const harnessCommand = 'cargo test -p rustok-blog --lib services::comment_projection::tests';
  const hostRegistrationHarnessCommand = 'cargo test -p rustok-blog --lib module::tests::module_registers_comment_projection_handler_with_host_routing';
  const dispatcherHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test event_dispatcher_routes_registered_handler_and_commits_projection -- --exact';
  const concurrencyHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test concurrent_created_events_converge_without_lost_updates -- --exact';
  const postgresHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test';
  const restartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test';
  const processRestartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test restarted_process_reuses_delivery_ledger_without_reapplying_counter -- --exact';

  const handler = `
const BLOG_POST_TARGET_TYPE: &str = "blog_post";
struct CommentProjectionChange
fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>
DomainEvent::CommentCreated
delta: 1
DomainEvent::CommentUpdated\nDomainEvent::CommentStatusChanged\nDomainEvent::CommentDeleted
delta: -1
fn projection_applied_delta(previous_delta: Option<i32>, current_delta: i32) -> i32
fn next_comment_count(comment_count: i32, delta: i32)
comment_count.saturating_add(delta).max(0)
let Some(change) = comment_projection_change(&envelope.event) else
let txn = self.db.begin().await?;
${missingPostLock ? '' : 'let Some(post = blog_post::Entity::find_by_id(change.post_id)\n.filter(blog_post::Column::TenantId.eq(envelope.tenant_id))\n.lock_exclusive()'}
${missingTenantScope ? '' : 'Column::TenantId.eq(envelope.tenant_id)'}
${missingDeliveryOrdering ? '' : 'Column::PostId.eq(change.post_id)\nColumn::CommentId.eq(change.comment_id)\norder_by_desc(blog_comment_projection_delivery::Column::EventId)\ndelivery.event_id >= envelope.id'}
${missingStateDelta ? '' : 'let applied_delta = projection_applied_delta(\nlet next_comment_count = next_comment_count(post.comment_count, applied_delta);\n'}
let post_updated =
${missingTenantScope ? '' : 'Column::TenantId.eq(envelope.tenant_id)'}
Column::CommentCount.eq(post.comment_count)
delta: Set(change.delta)
OnConflict::column(blog_comment_projection_delivery::Column::EventId)
.do_nothing()
.insert(&txn)
${missingOutbox ? '' : 'if post_updated {\nDomainEvent::ReindexRequested\n.publish_in_tx('}
txn.commit().await?;
impl EventHandler for BlogCommentProjectionHandler
fn handles(&self, event: &DomainEvent) -> bool {
  comment_projection_change(event).is_some()
}
async fn handle(&self, envelope: &EventEnvelope)
#[cfg(test)]
fn classifies_blog_comment_lifecycle_events()
fn ignores_non_blog_targets_and_unrelated_events()
fn projection_delta_tracks_comment_state_not_delivery_order()
fn counter_transition_is_non_negative_and_does_not_touch_business_revision()
`;
  write(root, handlerPath, handler);

  if (!missingPostgresHarness) {
    const dispatcherSource = missingDispatcherCase
      ? ''
      : `
async fn event_dispatcher_routes_registered_handler_and_commits_projection()
let extensions = ModuleRuntimeExtensions::default();
BlogModule.register_event_listeners(&mut registry, &context);
let bus = EventBus::new();
let mut dispatcher = EventDispatcher::with_config(
fail_fast: true
max_concurrent: 1
retry_count: 0
dispatcher.register_boxed(handler);
assert_eq!(dispatcher.handler_count(), 1);
let running = dispatcher.start();
running.bus().publish_envelope(envelope.clone());
${missingDispatcherWait ? '' : 'wait_for_dispatch_commit(&test_db.db, envelope.id).await?;'}
running.stop();
async fn wait_for_dispatch_commit(db: &DatabaseConnection, event_id: Uuid)
tokio::time::timeout(Duration::from_secs(5)
count_delivery(db, event_id).await? == 1
event dispatcher did not commit delivery
`;
    const concurrencySource = missingConcurrencyCase
      ? ''
      : `
const CONCURRENT_PROJECTION_DELIVERIES: usize = 4;
database_url: String
async fn isolated_connection(&self)
async fn concurrent_created_events_converge_without_lost_updates()
${missingConcurrencyBarrier ? '' : 'Arc::new(Barrier::new(envelopes.len()))'}
let db = test_db.isolated_connection().await?;
tasks.push(tokio::spawn(async move {
barrier.wait().await;
CONCURRENT_PROJECTION_DELIVERIES as i32
count_all_deliveries(&test_db.db).await?
`;
    write(
      root,
      postgresHarnessPath,
      `
const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
struct PostgresBlogProjectionTestDb
CREATE SCHEMA
DROP SCHEMA IF EXISTS
.max_connections(1)
SET search_path TO "{schema_name}"${publicSearchPathFallback ? ', public' : ''}
async fn duplicate_delivery_updates_counter_and_outbox_once()
handler.handle(&envelope).await?;
${dispatcherSource}
${concurrencySource}
async fn update_and_status_events_advance_projection_cursor_without_count_change()
DomainEvent::CommentUpdated
DomainEvent::CommentStatusChanged
async fn delete_before_create_stays_non_negative_and_replays_in_order()
DomainEvent::CommentDeleted
let comment_id = Uuid::new_v4();
comment_id,
async fn missing_post_replay_commits_only_after_source_appears()
missing Blog post must keep the delivery retryable
${missingRollbackCase ? '' : 'async fn outbox_failure_rolls_back_counter_and_delivery_before_retry()'}
DROP TABLE sys_events
missing outbox table must fail the projection transaction
create_outbox_table(&test_db.db).await?;
CREATE TABLE blog_comment_projection_deliveries
CREATE TABLE sys_events
count_delivery(&test_db.db, envelope.id)
count_outbox_events(&test_db.db)
`,
    );
  }

  if (!missingRestartHarness) {
    const processParentSource = missingProcessRestartCase
      ? ''
      : `
async fn restarted_process_reuses_delivery_ledger_without_reapplying_counter()
run_projection_worker(
${singleProcessWorkerInvocation ? '' : 'run_projection_worker('}
load_post_state(&test_db.db, tenant_id, post_id).await?, (1, 2)
count_delivery(&test_db.db, envelope.id).await?, 1
count_outbox_events(&test_db.db).await?, 1
`;
    const processWorkerSource = missingProcessWorker
      ? ''
      : `
async fn process_restart_worker_applies_envelope_from_env()
if env::var_os(PROCESS_WORKER_ENV).is_none()
let event_id = required_uuid(PROCESS_EVENT_ENV)?;
envelope.id = event_id;
envelope.correlation_id = event_id;
`;
    write(
      root,
      restartHarnessPath,
      `
const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const PROCESS_WORKER_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_WORKER";
const PROCESS_EVENT_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_EVENT_ID";
struct PostgresBlogProjectionRestartTestDb
database_url: String
async fn restarted_connection(&self)
SET search_path TO "{schema_name}"${publicSearchPathFallback ? ', public' : ''}
async fn restarted_handler_reuses_delivery_ledger_without_reapplying_counter()
let first_handler = BlogCommentProjectionHandler::new(test_db.db.clone());
first_handler.handle(&envelope).await?;
drop(first_handler);
${missingRestartConnection ? '' : 'let restarted_db = test_db.restarted_connection().await?;'}
let restarted_handler = BlogCommentProjectionHandler::new(restarted_db.clone());
restarted_handler.handle(&envelope).await?;
load_post_state(&restarted_db, tenant_id, post_id).await?, (1, 2)
count_delivery(&restarted_db, envelope.id).await?, 1
count_outbox_events(&restarted_db).await?, 1
${processParentSource}
${processWorkerSource}
fn run_projection_worker(
Command::new(env::current_exe()?)
.arg("--exact")
.arg("process_restart_worker_applies_envelope_from_env")
.env(PROCESS_WORKER_ENV, "1")
Blog projection restart worker exited with status
CREATE TABLE blog_comment_projection_deliveries
CREATE TABLE sys_events
`,
    );
  }

  write(root, serviceExportPath, 'pub use comment_projection::BlogCommentProjectionHandler;');
  write(root, entityPath, `
#[sea_orm(table_name = "blog_comment_projection_deliveries")]
#[sea_orm(primary_key, auto_increment = false)]
pub event_id: Uuid
pub tenant_id: Uuid
pub comment_id: Uuid
pub post_id: Uuid
pub delta: i32
`);
  write(root, migrationPath, `
BlogCommentProjectionDeliveries::EventId
.primary_key()
BlogCommentProjectionDeliveries::TenantId
BlogCommentProjectionDeliveries::PostId
idx_blog_comment_projection_deliveries_tenant_post
`);
  write(root, migrationRegistryPath, `
mod m20260716_000001_create_blog_comment_projection_deliveries;
Box::new(m20260716_000001_create_blog_comment_projection_deliveries::Migration)
`);

  const registrationSource = missingRegistration
    ? ''
    : `fn register_event_listeners(
registry.register(services::BlogCommentProjectionHandler::new(ctx.db.clone()));`;
  const hostHarnessSource = missingHostHarness
    ? ''
    : `
#[tokio::test]
async fn module_registers_comment_projection_handler_with_host_routing() {
let mut registry = ModuleEventListenerRegistry::new();
BlogModule.register_event_listeners(&mut registry, &context);
let handlers = registry.into_handlers();
assert_eq!(handlers.len(), 1);
assert_eq!(handler.name(), "blog_comment_projection");
assert!(handler.handles(&blog_created));
assert!(handler.handles(&blog_updated));
assert!(handler.handles(&blog_status_changed));
assert!(handler.handles(&blog_deleted));
assert!(!handler.handles(&forum_created));
}`;
  write(root, modulePath, `${registrationSource}\n${hostHarnessSource}`);

  const sourceHarnessCases = [
    'shared_created_updated_status_deleted_classifier',
    'non_blog_target_rejection',
    'projection_delta_tracks_comment_state_not_delivery_order',
    'counter_transition_is_non_negative_and_does_not_touch_business_revision',
  ];

  const evidence = {
    schema_version: 6,
    module: 'blog',
    surface: 'comments_event_projection',
    status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
    compile_policy: 'not_run_by_request',
    runtime_status: 'pending',
    owner: 'rustok-blog',
    provider: 'rustok-comments',
    events: ['comment.created', 'comment.updated', 'comment.status_changed', 'comment.deleted'],
    production_contract: {
      handler: handlerPath,
      service_export: serviceExportPath,
      delivery_entity: entityPath,
      delivery_migration: migrationPath,
      migration_registry: migrationRegistryPath,
      module_registration: modulePath,
      consumer_registry: registryPath,
    },
    source_harness: {
      status: harnessStatusDrift ? 'executed' : 'executable_no_run',
      path: handlerPath,
      module: 'services::comment_projection::tests',
      command: harnessCommand,
      cases: sourceHarnessCases,
    },
    host_registration_harness: {
      status: hostHarnessStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: hostHarnessStatusDrift ? 'passed' : 'not_run',
      path: modulePath,
      module: 'module::tests',
      command: hostRegistrationHarnessCommand,
      scope: 'module_registry_handler_identity_and_routing_only',
      cases: ['module_registers_comment_projection_handler_with_host_routing'],
    },
    dispatcher_harness: {
      status: dispatcherStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: dispatcherStatusDrift ? 'passed' : 'not_run',
      path: postgresHarnessPath,
      environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
      command: dispatcherHarnessCommand,
      isolation: 'unique_schema_one_connection_pool',
      scope: 'event_bus_dispatcher_module_registered_handler_transactional_commit',
      cases: ['event_dispatcher_routes_registered_handler_and_commits_projection'],
    },
    concurrency_harness: {
      status: concurrencyStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: concurrencyStatusDrift ? 'passed' : 'not_run',
      path: postgresHarnessPath,
      environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
      command: concurrencyHarnessCommand,
      isolation: 'unique_schema_four_independent_connections_barrier',
      scope: 'concurrent_unique_envelopes_same_post_final_counter_delivery_outbox',
      cases: ['concurrent_created_events_converge_without_lost_updates'],
    },
    postgres_harness: {
      status: postgresStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: postgresStatusDrift ? 'passed' : 'not_run',
      path: postgresHarnessPath,
      environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
      command: postgresHarnessCommand,
      isolation: 'unique_schema_one_connection_pool',
      cases: [
        'duplicate_delivery_updates_counter_and_outbox_once',
        'delete_before_create_stays_non_negative_and_replays_in_order',
        'missing_post_replay_commits_only_after_source_appears',
        'outbox_failure_rolls_back_counter_and_delivery_before_retry',
      ],
    },
    restart_harness: {
      status: restartStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: restartStatusDrift ? 'passed' : 'not_run',
      path: restartHarnessPath,
      environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
      command: restartHarnessCommand,
      isolation: 'unique_schema_new_connection',
      scope: 'same_process_new_connection_and_handler',
      cases: ['restarted_handler_reuses_delivery_ledger_without_reapplying_counter'],
    },
    process_restart_harness: {
      status: processRestartStatusDrift ? 'executed' : 'executable_no_run',
      runtime_status: processRestartStatusDrift ? 'passed' : 'not_run',
      path: restartHarnessPath,
      environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
      command: processRestartHarnessCommand,
      isolation: 'unique_schema_two_sequential_test_processes_same_envelope',
      scope: 'os_process_reinstantiation_durable_delivery_replay',
      non_claim: 'does_not_prove_full_server_host_restart_or_record_execution',
      cases: [
        'restarted_process_reuses_delivery_ledger_without_reapplying_counter',
        'process_restart_worker_applies_envelope_from_env',
      ],
    },
    cases: [
      { name: 'shared_event_classifier' },
      { name: 'blog_post_target_filter' },
      { name: 'created_deleted_delta' },
      { name: 'envelope_idempotency' },
      { name: 'atomic_counter_delivery_outbox' },
      { name: 'tenant_scoped_row_lock' },
      { name: 'comment_lifecycle_ordering' },
      { name: 'missing_post_retry' },
      { name: 'non_negative_count' },
      { name: 'host_registration_routing_harness' },
      { name: 'postgres_event_dispatcher_delivery' },
      { name: 'postgres_concurrent_unique_deliveries' },
      { name: 'postgres_duplicate_delivery' },
      { name: 'postgres_out_of_order_delete_create' },
      { name: 'postgres_missing_post_recovery' },
      { name: 'postgres_outbox_rollback_recovery' },
      { name: 'postgres_restart_replay' },
      { name: 'postgres_process_restart_replay' },
      { name: 'module_listener_registration' },
    ],
  };
  write(root, evidencePath, JSON.stringify(evidence, null, 2));

  write(root, registryPath, JSON.stringify({
    schema_version: 15,
    evidence: { comments_event_projection: evidencePath },
    verification_chain: {
      source_gates: {
        comments_event_projection: {
          unit_test: handlerPath,
          ...(missingPostgresRegistration ? {} : { postgres_test: postgresHarnessPath }),
          ...(missingRestartRegistration ? {} : { restart_test: restartHarnessPath }),
        },
      },
    },
    event_projection: {
      provider: 'comments',
      events: ['comment.created', 'comment.updated', 'comment.status_changed', 'comment.deleted'],
      handler: 'BlogCommentProjectionHandler',
      delivery_ledger: 'blog_comment_projection_deliveries',
      status: 'implemented_static_only',
      runtime_status: 'pending',
      snapshot_invalidation: {
        source: 'blog_comment_projection_deliveries',
        cursor: 'latest processed lifecycle event_id for the tenant/post',
        keying: 'public snapshot identity includes projection_event_id',
        redis_enumeration: false,
      },
      source_harness: {
        path: handlerPath,
        status: 'executable_no_run',
        command: harnessCommand,
      },
      postgres_harness: {
        path: postgresHarnessPath,
        status: postgresStatusDrift ? 'executed' : 'executable_no_run',
        runtime_status: postgresStatusDrift ? 'passed' : 'not_run',
        environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
        command: postgresHarnessCommand,
      },
      restart_harness: {
        path: restartHarnessPath,
        status: restartStatusDrift ? 'executed' : 'executable_no_run',
        runtime_status: restartStatusDrift ? 'passed' : 'not_run',
        environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
        command: restartHarnessCommand,
      },
    },
  }, null, 2));

  write(root, planPath(), [
    'Blog FBA registry schema v15 and Comments projection evidence schema v6',
    'derived Comments counters that preserve Blog business',
    'source-level',
    'runtime/remote evidence is still pending',
  ].join(' '));

  return root;
}

function planPath() {
  return 'crates/modules/rustok-blog/docs/implementation-plan-current.md';
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve('.'),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function expectRejected(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    if (pattern) assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('accepts the canonical Comments-to-Blog projection contract', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a projection without tenant scope', () => {
  expectRejected({ missingTenantScope: true }, /tenant/);
});

test('rejects a projection without Blog-post row locking', () => {
  expectRejected({ missingPostLock: true }, /post row lock|row-lock/);
});

test('rejects a projection without per-comment event ordering', () => {
  expectRejected({ missingDeliveryOrdering: true }, /per-comment delivery ordering|ordering/);
});

test('rejects a projection without state-based lifecycle delta', () => {
  expectRejected({ missingStateDelta: true }, /state-based delta|projection_delta/);
});

test('rejects a projection without transactional outbox publication', () => {
  expectRejected({ missingOutbox: true }, /ReindexRequested|outbox/);
});

test('rejects missing module event-listener registration', () => {
  expectRejected({ missingRegistration: true }, /register_event_listeners/);
});

test('rejects missing module registration and routing harness', () => {
  expectRejected(
    { missingHostHarness: true },
    /missing async fn module_registers_comment_projection_handler_with_host_routing/,
  );
});

test('rejects a missing EventDispatcher delivery case', () => {
  expectRejected(
    { missingDispatcherCase: true },
    /missing async fn event_dispatcher_routes_registered_handler_and_commits_projection/,
  );
});

test('rejects EventDispatcher coverage without durable delivery wait', () => {
  expectRejected(
    { missingDispatcherWait: true },
    /missing wait_for_dispatch_commit\(&test_db.db, envelope.id\).await\?;/,
  );
});

test('rejects a missing concurrent projection case', () => {
  expectRejected(
    { missingConcurrencyCase: true },
    /missing async fn concurrent_created_events_converge_without_lost_updates/,
  );
});

test('rejects concurrent projection coverage without a shared barrier', () => {
  expectRejected(
    { missingConcurrencyBarrier: true },
    /missing Arc::new\(Barrier::new\(envelopes.len\(\)\)\)/,
  );
});

test('rejects a missing PostgreSQL harness source', () => {
  expectRejected({ missingPostgresHarness: true }, /expected file is missing/);
});

test('rejects a PostgreSQL harness without rollback coverage', () => {
  expectRejected(
    { missingRollbackCase: true },
    /missing async fn outbox_failure_rolls_back_counter_and_delivery_before_retry/,
  );
});

test('rejects PostgreSQL harnesses that can fall back to public tables', () => {
  expectRejected(
    { publicSearchPathFallback: true },
    /forbidden SET search_path TO "\{schema_name\}", public/,
  );
});

test('rejects a registry without the PostgreSQL target', () => {
  expectRejected({ missingPostgresRegistration: true }, /PostgreSQL test path drift/);
});

test('rejects a missing restart harness source', () => {
  expectRejected({ missingRestartHarness: true }, /expected file is missing/);
});

test('rejects restart coverage that reuses the original connection', () => {
  expectRejected(
    { missingRestartConnection: true },
    /missing let restarted_db = test_db.restarted_connection\(\).await\?;/,
  );
});

test('rejects a missing process restart parent case', () => {
  expectRejected(
    { missingProcessRestartCase: true },
    /missing async fn restarted_process_reuses_delivery_ledger_without_reapplying_counter/,
  );
});

test('rejects a missing process restart worker', () => {
  expectRejected(
    { missingProcessWorker: true },
    /missing async fn process_restart_worker_applies_envelope_from_env/,
  );
});

test('rejects process restart coverage with only one child process', () => {
  expectRejected(
    { singleProcessWorkerInvocation: true },
    /expected 2 occurrences of run_projection_worker\(/,
  );
});

test('rejects runtime status promotion without execution', () => {
  expectRejected({ statusDrift: true }, /status drift/);
});

test('rejects source harness execution promotion without execution', () => {
  expectRejected({ harnessStatusDrift: true }, /source harness drift/);
});

test('rejects host registration harness execution promotion without execution', () => {
  expectRejected({ hostHarnessStatusDrift: true }, /host registration harness drift/);
});

test('rejects dispatcher harness execution promotion without execution', () => {
  expectRejected({ dispatcherStatusDrift: true }, /dispatcher_harness/);
});

test('rejects concurrency harness execution promotion without execution', () => {
  expectRejected({ concurrencyStatusDrift: true }, /concurrency_harness/);
});

test('rejects PostgreSQL harness execution promotion without execution', () => {
  expectRejected({ postgresStatusDrift: true }, /PostgreSQL harness drift/);
});

test('rejects restart harness execution promotion without execution', () => {
  expectRejected({ restartStatusDrift: true }, /restart harness drift/);
});

test('rejects process restart harness execution promotion without execution', () => {
  expectRejected({ processRestartStatusDrift: true }, /process restart harness drift/);
});

test('rejects stale optimistic projection artifacts in the evidence contract', () => {
  const root = fixture();
  try {
    const evidencePath = path.join(root, 'crates/modules/rustok-blog/contracts/evidence/blog-comments-event-projection.json');
    const evidence = JSON.parse(readFile(root, evidencePath));
    evidence.cases.push({ name: 'bounded_optimistic_retry_policy' });
    writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /obsolete case bounded_optimistic_retry_policy/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function readFile(root, target) {
  return JSON.parse(readFileSync(target, 'utf8'));
}
