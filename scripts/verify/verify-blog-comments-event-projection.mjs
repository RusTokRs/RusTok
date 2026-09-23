#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve('.');
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return '';
  }
  return readFileSync(target, 'utf8');
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireNoMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function requireCount(source, marker, expected, label) {
  const count = source.split(marker).length - 1;
  if (count !== expected) {
    failures.push(`${label}: expected ${expected} occurrences of ${marker}, found ${count}`);
  }
}

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
const planPath = 'crates/modules/rustok-blog/docs/implementation-plan-current.md';
const harnessCommand = 'cargo test -p rustok-blog --lib services::comment_projection::tests';
const hostRegistrationHarnessCommand = 'cargo test -p rustok-blog --lib module::tests::module_registers_comment_projection_handler_with_host_routing';
const dispatcherHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test event_dispatcher_routes_registered_handler_and_commits_projection -- --exact';
const concurrencyHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test concurrent_created_events_converge_without_lost_updates -- --exact';
const postgresHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test';
const restartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test';
const processRestartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test restarted_process_reuses_delivery_ledger_without_reapplying_counter -- --exact';
const postgresHarnessEnvironment = 'RUSTOK_BLOG_TEST_DATABASE_URL';

const handler = read(handlerPath);
const postgresHarness = read(postgresHarnessPath);
const restartHarness = read(restartHarnessPath);
const serviceExport = read(serviceExportPath);
const entity = read(entityPath);
const migration = read(migrationPath);
const migrationRegistry = read(migrationRegistryPath);
const moduleSource = read(modulePath);
const plan = read(planPath);
let evidence = null;
let registry = null;

try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}
try {
  registry = JSON.parse(read(registryPath));
} catch (error) {
  failures.push(`${registryPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  'const BLOG_POST_TARGET_TYPE: &str = "blog_post";',
  'struct CommentProjectionChange',
  'fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>',
  'DomainEvent::CommentCreated',
  'DomainEvent::CommentUpdated',
  'DomainEvent::CommentStatusChanged',
  'delta: 1',
  'DomainEvent::CommentDeleted',
  'delta: -1',
  'fn projection_applied_delta(',
  'fn next_comment_count(comment_count: i32, delta: i32)',
  'comment_count.saturating_add(delta).max(0)',
  'let Some(change) = comment_projection_change(&envelope.event) else',
  'let txn = self.db.begin().await?;',
  'lock_exclusive()',
  'Column::TenantId.eq(envelope.tenant_id)',
  'Column::PostId.eq(change.post_id)',
  'Column::CommentId.eq(change.comment_id)',
  'order_by_desc(blog_comment_projection_delivery::Column::EventId)',
  'delivery.event_id >= envelope.id',
  'let applied_delta = projection_applied_delta(',
  'let next_comment_count = next_comment_count(post.comment_count, applied_delta);',
  'let post_updated =',
  'Column::CommentCount.eq(post.comment_count)',
  'delta: Set(change.delta)',
  'OnConflict::column(blog_comment_projection_delivery::Column::EventId)',
  '.do_nothing()',
  'if post_updated',
  'DomainEvent::ReindexRequested',
  '.publish_in_tx(',
  'txn.commit().await?;',
  'impl EventHandler for BlogCommentProjectionHandler',
  'comment_projection_change(event).is_some()',
  '#[cfg(test)]',
  'fn classifies_blog_comment_lifecycle_events()',
  'fn ignores_non_blog_targets_and_unrelated_events()',
  'fn projection_delta_tracks_comment_state_not_delivery_order()',
  'fn counter_transition_is_non_negative_and_does_not_touch_business_revision()',
  'projection_revision',
]) {
  requireMarker(handler, marker, handlerPath);
}

for (const marker of [
  'MAX_PROJECTION_UPDATE_ATTEMPTS',
  'ProjectionUpdateDecision',
  'projection_update_decision(',
  'for attempt_index in 0..MAX_PROJECTION_UPDATE_ATTEMPTS',
  'optimistic_retry_policy_applies_success_without_retry',
  'optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict',
  'blog_post::Column::Version',
  'blog_post::Column::UpdatedAt',
]) {
  requireNoMarker(handler, marker, handlerPath);
}

const handlesStart = handler.indexOf('fn handles(&self, event: &DomainEvent) -> bool');
const handleStart = handler.indexOf('async fn handle(&self, envelope: &EventEnvelope)', handlesStart);
if (handlesStart === -1 || handleStart === -1) {
  failures.push(`${handlerPath}: missing EventHandler handles/handle boundary`);
} else {
  const handlesBody = handler.slice(handlesStart, handleStart);
  requireMarker(handlesBody, 'comment_projection_change(event).is_some()', `${handlerPath}: handles`);
  requireNoMarker(handlesBody, 'matches!(', `${handlerPath}: handles`);
}

const projectStart = handler.indexOf('async fn project(&self, envelope: &EventEnvelope)');
const projectEnd = handler.indexOf('impl EventHandler for BlogCommentProjectionHandler', projectStart);
const projectBody = projectStart === -1 || projectEnd === -1
  ? ''
  : handler.slice(projectStart, projectEnd);
requireNoMarker(
  projectBody,
  'DomainEvent::BlogPostUpdated',
  `${handlerPath}: project`,
);
const txnStart = handler.indexOf('let txn = self.db.begin().await?;', projectStart);
const postLock = handler.indexOf('.lock_exclusive()', txnStart);
const latestQuery = handler.indexOf(
  'order_by_desc(blog_comment_projection_delivery::Column::EventId)',
  postLock,
);
const lifecycleGate = handler.indexOf(
  'delivery.event_id >= envelope.id',
  latestQuery,
);
const appliedDelta = handler.indexOf(
  'let applied_delta = projection_applied_delta(',
  lifecycleGate,
);
const postUpdate = handler.indexOf('let post_updated =', appliedDelta);
const deliveryInsert = handler.indexOf(
  'OnConflict::column(blog_comment_projection_delivery::Column::EventId)',
  postUpdate,
);
const reindex = handler.indexOf('DomainEvent::ReindexRequested', deliveryInsert);
const commit = handler.indexOf('txn.commit().await?;', reindex);

for (const [name, index] of [
  ['project', projectStart],
  ['transaction', txnStart],
  ['post row lock', postLock],
  ['per-comment delivery ordering', latestQuery],
  ['older-delivery guard', lifecycleGate],
  ['state-based delta', appliedDelta],
  ['counter update', postUpdate],
  ['delivery insert', deliveryInsert],
  ['reindex publication', reindex],
  ['transaction commit', commit],
]) {
  if (index === -1) failures.push(`${handlerPath}: missing ${name} ordering marker`);
}

if (
  projectStart !== -1 &&
  [txnStart, postLock, latestQuery, lifecycleGate, appliedDelta, postUpdate, deliveryInsert, reindex, commit]
    .some((index) => index === -1)
) {
  failures.push(`${handlerPath}: incomplete projection ordering chain`);
} else if (
  projectStart !== -1 &&
  !(projectStart < txnStart &&
    txnStart < postLock &&
    postLock < latestQuery &&
    latestQuery < lifecycleGate &&
    lifecycleGate < appliedDelta &&
    appliedDelta < postUpdate &&
    postUpdate < deliveryInsert &&
    deliveryInsert < reindex &&
    reindex < commit)
) {
  failures.push(`${handlerPath}: expected row-lock -> ordering -> state-delta -> counter -> delivery -> reindex -> commit sequence`);
}

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'const CONCURRENT_PROJECTION_DELIVERIES: usize = 4;',
  'struct PostgresBlogProjectionTestDb',
  'database_url: String',
  'async fn isolated_connection(&self)',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  '.max_connections(1)',
  'SET search_path TO "{schema_name}"',
  'async fn duplicate_delivery_updates_counter_and_outbox_once()',
  'async fn event_dispatcher_routes_registered_handler_and_commits_projection()',
  'let extensions = ModuleRuntimeExtensions::default();',
  'BlogModule.register_event_listeners(&mut registry, &context);',
  'let bus = EventBus::new();',
  'let mut dispatcher = EventDispatcher::with_config(',
  'fail_fast: true',
  'max_concurrent: 1',
  'retry_count: 0',
  'dispatcher.register_boxed(handler);',
  'assert_eq!(dispatcher.handler_count(), 1);',
  'let running = dispatcher.start();',
  'running.bus().publish_envelope(envelope.clone())?;',
  'wait_for_dispatch_commit(&test_db.db, envelope.id).await?;',
  'async fn concurrent_created_events_converge_without_lost_updates()',
  'Arc::new(Barrier::new(envelopes.len()))',
  'let db = test_db.isolated_connection().await?;',
  'tasks.push(tokio::spawn(async move {',
  'barrier.wait().await;',
  'CONCURRENT_PROJECTION_DELIVERIES as i32',
  'count_all_deliveries(&test_db.db).await?',
  'async fn delete_before_create_stays_non_negative_and_replays_in_order()',
  'DomainEvent::CommentDeleted',
  'async fn missing_post_replay_commits_only_after_source_appears()',
  'missing Blog post must keep the delivery retryable',
  'async fn outbox_failure_rolls_back_counter_and_delivery_before_retry()',
  'async fn update_and_status_events_advance_projection_cursor_without_count_change()',
  'DROP TABLE sys_events',
  'missing outbox table must fail the projection transaction',
  'create_outbox_table(&test_db.db).await?;',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
  'count_delivery(&test_db.db, envelope.id)',
  'count_outbox_events(&test_db.db)',
]) {
  requireMarker(postgresHarness, marker, postgresHarnessPath);
}

for (const marker of [
  'optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears',
  'install_retry_limit_probe',
  'force_blog_projection_retry_limit',
  'blog_projection_retry_attempts',
  'EXPECTED_RETRY_LIMIT_ATTEMPTS',
]) {
  requireNoMarker(postgresHarness, marker, postgresHarnessPath);
}
requireNoMarker(postgresHarness, '#[ignore]', postgresHarnessPath);
requireNoMarker(postgresHarness, 'runtime_verified', postgresHarnessPath);
requireNoMarker(
  postgresHarness,
  'SET search_path TO "{schema_name}", public',
  postgresHarnessPath,
);

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'const PROCESS_WORKER_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_WORKER";',
  'const PROCESS_EVENT_ENV: &str = "RUSTOK_BLOG_PROCESS_RESTART_EVENT_ID";',
  'struct PostgresBlogProjectionRestartTestDb',
  'database_url: String',
  'async fn restarted_connection(&self)',
  'SET search_path TO "{schema_name}"',
  'async fn restarted_handler_reuses_delivery_ledger_without_reapplying_counter()',
  'let first_handler = BlogCommentProjectionHandler::new(test_db.db.clone());',
  'first_handler.handle(&envelope).await?;',
  'drop(first_handler);',
  'let restarted_db = test_db.restarted_connection().await?;',
  'let restarted_handler = BlogCommentProjectionHandler::new(restarted_db.clone());',
  'restarted_handler.handle(&envelope).await?;',
  'load_post_state(&restarted_db, tenant_id, post_id).await?',
  'count_delivery(&restarted_db, envelope.id).await?, 1',
  'count_outbox_events(&restarted_db).await?, 1',
  'async fn restarted_process_reuses_delivery_ledger_without_reapplying_counter()',
  'async fn process_restart_worker_applies_envelope_from_env()',
  'if env::var_os(PROCESS_WORKER_ENV).is_none()',
  'let event_id = required_uuid(PROCESS_EVENT_ENV)?;',
  'envelope.id = event_id;',
  'envelope.correlation_id = event_id;',
  'fn run_projection_worker(',
  'Command::new(env::current_exe()?)',
  '.arg("--exact")',
  '.arg("process_restart_worker_applies_envelope_from_env")',
  '.env(PROCESS_WORKER_ENV, "1")',
  'Blog projection restart worker exited with status',
  'load_post_state(&test_db.db, tenant_id, post_id).await?',
  'count_delivery(&test_db.db, envelope.id).await?, 1',
  'count_outbox_events(&test_db.db).await?, 1',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
]) {
  requireMarker(restartHarness, marker, restartHarnessPath);
}
const processParentStart = restartHarness.indexOf(
  'async fn restarted_process_reuses_delivery_ledger_without_reapplying_counter()',
);
const processWorkerStart = restartHarness.indexOf(
  'async fn process_restart_worker_applies_envelope_from_env()',
  processParentStart,
);
if (processParentStart === -1 || processWorkerStart === -1) {
  failures.push(`${restartHarnessPath}: missing process restart parent/worker boundary`);
} else {
  requireCount(
    restartHarness.slice(processParentStart, processWorkerStart),
    'run_projection_worker(',
    2,
    `${restartHarnessPath}: process restart parent`,
  );
}
requireNoMarker(restartHarness, '#[ignore]', restartHarnessPath);
requireNoMarker(restartHarness, 'runtime_verified', restartHarnessPath);
requireNoMarker(
  restartHarness,
  'SET search_path TO "{schema_name}", public',
  restartHarnessPath,
);

for (const marker of [
  '#[sea_orm(table_name = "blog_comment_projection_deliveries")]',
  '#[sea_orm(primary_key, auto_increment = false)]',
  'pub event_id: Uuid',
  'pub tenant_id: Uuid',
  'pub comment_id: Uuid',
  'pub post_id: Uuid',
  'pub delta: i32',
]) {
  requireMarker(entity, marker, entityPath);
}

for (const marker of [
  'BlogCommentProjectionDeliveries::EventId',
  '.primary_key()',
  'BlogCommentProjectionDeliveries::TenantId',
  'BlogCommentProjectionDeliveries::PostId',
  'idx_blog_comment_projection_deliveries_tenant_post',
]) {
  requireMarker(migration, marker, migrationPath);
}

for (const marker of [
  'mod m20260716_000001_create_blog_comment_projection_deliveries;',
  'Box::new(m20260716_000001_create_blog_comment_projection_deliveries::Migration)',
]) {
  requireMarker(migrationRegistry, marker, migrationRegistryPath);
}

requireMarker(serviceExport, 'pub use comment_projection::BlogCommentProjectionHandler;', serviceExportPath);

for (const marker of [
  'fn register_event_listeners(',
  'registry.register(services::BlogCommentProjectionHandler::new(ctx.db.clone()));',
  '#[tokio::test]',
  'async fn module_registers_comment_projection_handler_with_host_routing()',
  'let mut registry = ModuleEventListenerRegistry::new();',
  'BlogModule.register_event_listeners(&mut registry, &context);',
  'let handlers = registry.into_handlers();',
  'assert_eq!(handlers.len(), 1);',
  'assert_eq!(handler.name(), "blog_comment_projection");',
  'assert!(handler.handles(&blog_created));',
  'assert!(handler.handles(&blog_updated));',
  'assert!(handler.handles(&blog_status_changed));',
  'assert!(handler.handles(&blog_deleted));',
  'assert!(!handler.handles(&forum_created));',
]) {
  requireMarker(moduleSource, marker, modulePath);
}
requireNoMarker(moduleSource, 'handler.handle(&', `${modulePath}: host registration harness`);

if (evidence) {
  if (evidence.schema_version !== 7) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_event_projection' ||
    evidence.owner !== 'rustok-blog' ||
    evidence.provider !== 'rustok-comments'
  ) {
    failures.push(`${evidencePath}: identity drift`);
  }
  if (evidence.status !== 'source_verified_no_compile') failures.push(`${evidencePath}: status drift`);
  if (evidence.compile_policy !== 'not_run_by_request') failures.push(`${evidencePath}: compile policy drift`);
  if (evidence.runtime_status !== 'pending') failures.push(`${evidencePath}: runtime status drift`);

  const contract = evidence.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    handler: handlerPath,
    service_export: serviceExportPath,
    delivery_entity: entityPath,
    delivery_migration: migrationPath,
    migration_registry: migrationRegistryPath,
    module_registration: modulePath,
    consumer_registry: registryPath,
  })) {
    if (contract[key] !== expected) failures.push(`${evidencePath}: ${key} drift`);
  }

  const sourceHarness = evidence.source_harness ?? {};
  if (
    sourceHarness.status !== 'executable_no_run' ||
    sourceHarness.path !== handlerPath ||
    sourceHarness.module !== 'services::comment_projection::tests' ||
    sourceHarness.command !== harnessCommand
  ) {
    failures.push(`${evidencePath}: source harness drift`);
  }
  if (
    [...(sourceHarness.cases ?? [])].sort().join('|') !==
    [
      'shared_created_updated_status_deleted_classifier',
      'non_blog_target_rejection',
      'projection_delta_tracks_comment_state_not_delivery_order',
      'counter_transition_is_non_negative_and_does_not_touch_business_revision',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: source harness case drift`);
  }

  const postgres = evidence.postgres_harness ?? {};
  if (
    postgres.status !== 'executable_no_run' ||
    postgres.runtime_status !== 'not_run' ||
    postgres.path !== postgresHarnessPath ||
    postgres.environment !== postgresHarnessEnvironment ||
    postgres.command !== postgresHarnessCommand ||
    postgres.isolation !== 'unique_schema_one_connection_pool'
  ) {
    failures.push(`${evidencePath}: PostgreSQL harness drift`);
  }
  if (
    [...(postgres.cases ?? [])].sort().join('|') !==
    [
      'duplicate_delivery_updates_counter_and_outbox_once',
      'delete_before_create_stays_non_negative_and_replays_in_order',
      'missing_post_replay_commits_only_after_source_appears',
      'outbox_failure_rolls_back_counter_and_delivery_before_retry',
      'update_and_status_events_advance_projection_cursor_without_count_change',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: PostgreSQL harness case drift`);
  }

  for (const [field, expectedCommand, expectedCase] of [
    ['dispatcher_harness', dispatcherHarnessCommand, 'event_dispatcher_routes_registered_handler_and_commits_projection'],
    ['concurrency_harness', concurrencyHarnessCommand, 'concurrent_created_events_converge_without_lost_updates'],
  ]) {
    const harness = evidence[field] ?? {};
    if (
      harness.status !== 'executable_no_run' ||
      harness.runtime_status !== 'not_run' ||
      harness.path !== postgresHarnessPath ||
      harness.environment !== postgresHarnessEnvironment ||
      harness.command !== expectedCommand ||
      ![...(harness.cases ?? [])].includes(expectedCase)
    ) {
      failures.push(`${evidencePath}: ${field} drift`);
    }
  }

  const restart = evidence.restart_harness ?? {};
  if (
    restart.status !== 'executable_no_run' ||
    restart.runtime_status !== 'not_run' ||
    restart.path !== restartHarnessPath ||
    restart.environment !== postgresHarnessEnvironment ||
    restart.command !== restartHarnessCommand ||
    restart.isolation !== 'unique_schema_new_connection' ||
    restart.scope !== 'same_process_new_connection_and_handler' ||
    [...(restart.cases ?? [])].join('|') !==
      'restarted_handler_reuses_delivery_ledger_without_reapplying_counter'
  ) {
    failures.push(`${evidencePath}: restart harness drift`);
  }

  const processRestart = evidence.process_restart_harness ?? {};
  if (
    processRestart.status !== 'executable_no_run' ||
    processRestart.runtime_status !== 'not_run' ||
    processRestart.path !== restartHarnessPath ||
    processRestart.environment !== postgresHarnessEnvironment ||
    processRestart.command !== processRestartHarnessCommand ||
    processRestart.isolation !== 'unique_schema_two_sequential_test_processes_same_envelope' ||
    processRestart.scope !== 'os_process_reinstantiation_durable_delivery_replay' ||
    processRestart.non_claim !== 'does_not_prove_full_server_host_restart_or_record_execution'
  ) {
    failures.push(`${evidencePath}: process restart harness drift`);
  }
  if (
    [...(processRestart.cases ?? [])].sort().join('|') !==
    [
      'restarted_process_reuses_delivery_ledger_without_reapplying_counter',
      'process_restart_worker_applies_envelope_from_env',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: process restart harness case drift`);
  }

  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    'blog_post_target_filter',
    'envelope_idempotency',
    'atomic_counter_delivery_outbox',
    'tenant_scoped_row_lock',
    'comment_lifecycle_ordering',
    'missing_post_retry',
    'non_negative_count',
    'host_registration_routing_harness',
    'postgres_event_dispatcher_delivery',
    'postgres_concurrent_unique_deliveries',
    'postgres_duplicate_delivery',
    'postgres_out_of_order_delete_create',
    'postgres_missing_post_recovery',
    'postgres_outbox_rollback_recovery',
    'postgres_restart_replay',
    'postgres_process_restart_replay',
    'module_listener_registration',
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }

  for (const forbidden of [
    'bounded_optimistic_retry_policy',
    'postgres_retry_limit_rollback_and_replay',
    'tenant_scoped_optimistic_update',
    'ProjectionUpdateDecision',
  ]) {
    if (cases.has(forbidden)) failures.push(`${evidencePath}: obsolete case ${forbidden}`);
  }
}

if (registry) {
  if (registry.schema_version !== 16) failures.push(`${registryPath}: schema_version drift`);
  if (registry.evidence?.comments_event_projection !== evidencePath) {
    failures.push(`${registryPath}: comments event projection evidence path drift`);
  }
  const projection = registry.event_projection ?? {};
  if (
    projection.provider !== 'comments' ||
    projection.handler !== 'BlogCommentProjectionHandler' ||
    projection.delivery_ledger !== 'blog_comment_projection_deliveries' ||
    projection.status !== 'implemented_static_only' ||
    projection.runtime_status !== 'pending'
  ) {
    failures.push(`${registryPath}: event projection metadata drift`);
  }
  if (
    projection.source_harness?.path !== handlerPath ||
    projection.source_harness?.status !== 'executable_no_run' ||
    projection.source_harness?.command !== harnessCommand
  ) {
    failures.push(`${registryPath}: event projection source harness drift`);
  }
  if (
    projection.postgres_harness?.path !== postgresHarnessPath ||
    projection.postgres_harness?.status !== 'executable_no_run' ||
    projection.postgres_harness?.runtime_status !== 'not_run' ||
    projection.postgres_harness?.environment !== postgresHarnessEnvironment ||
    projection.postgres_harness?.command !== postgresHarnessCommand
  ) {
    failures.push(`${registryPath}: event projection PostgreSQL harness drift`);
  }
  if (
    projection.restart_harness?.path !== restartHarnessPath ||
    projection.restart_harness?.status !== 'executable_no_run' ||
    projection.restart_harness?.runtime_status !== 'not_run' ||
    projection.restart_harness?.environment !== postgresHarnessEnvironment ||
    projection.restart_harness?.command !== restartHarnessCommand
  ) {
    failures.push(`${registryPath}: event projection restart harness drift`);
  }
}

for (const marker of [
  'Blog FBA registry schema v16 and Comments projection evidence schema v7',
  'derived Comments counters that preserve Blog business',
  'architecture/source level',
  'Runtime/remote evidence is still pending',
]) {
  requireMarker(plan, marker, planPath);
}
requireNoMarker(plan, 'ProjectionUpdateDecision', planPath);
requireNoMarker(plan, 'bounded optimistic retry', planPath);
requireNoMarker(plan, 'retry-limit', planPath);

if (failures.length > 0) {
  console.error('Blog comments event projection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog comments event projection classifier, row-lock ordering, delivery ledger, registration, dispatcher, concurrency, PostgreSQL recovery, connection restart, and process restart harnesses are consistent');
