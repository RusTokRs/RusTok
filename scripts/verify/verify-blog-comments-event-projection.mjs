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

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json';
const handlerPath = 'crates/rustok-blog/src/services/comment_projection.rs';
const postgresHarnessPath = 'crates/rustok-blog/tests/comment_projection_postgres_test.rs';
const restartHarnessPath = 'crates/rustok-blog/tests/comment_projection_restart_postgres_test.rs';
const serviceExportPath = 'crates/rustok-blog/src/services/mod.rs';
const entityPath = 'crates/rustok-blog/src/entities/blog_comment_projection_delivery.rs';
const migrationPath = 'crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs';
const migrationRegistryPath = 'crates/rustok-blog/src/migrations/mod.rs';
const modulePath = 'crates/rustok-blog/src/lib.rs';
const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessCommand = 'cargo test -p rustok-blog --lib services::comment_projection::tests';
const hostRegistrationHarnessCommand = 'cargo test -p rustok-blog --lib tests::module_registers_comment_projection_handler_with_host_routing';
const dispatcherHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test event_dispatcher_routes_registered_handler_and_commits_projection -- --exact';
const concurrencyHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test concurrent_created_events_converge_without_lost_updates -- --exact';
const retryLimitHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears -- --exact';
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
  'const MAX_PROJECTION_UPDATE_ATTEMPTS: usize = 8;',
  'struct CommentProjectionChange',
  'enum ProjectionUpdateDecision',
  'ProjectionUpdateDecision::Applied',
  'ProjectionUpdateDecision::Retry',
  'ProjectionUpdateDecision::LimitReached',
  'fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>',
  'DomainEvent::CommentCreated',
  'delta: 1',
  'DomainEvent::CommentDeleted',
  'delta: -1',
  'fn next_comment_projection_state(comment_count: i32, version: i32, delta: i32)',
  'comment_count.saturating_add(delta).max(0)',
  'version.saturating_add(1)',
  'fn projection_update_decision(',
  'attempt_index: usize',
  'rows_affected: u64',
  'else if attempt_index + 1 < MAX_PROJECTION_UPDATE_ATTEMPTS',
  'let Some(change) = comment_projection_change(&envelope.event) else',
  'let txn = self.db.begin().await?;',
  'blog_comment_projection_delivery::Entity::find_by_id(envelope.id)',
  'change.post_id',
  'change.delta',
  'event_id: Set(envelope.id)',
  'comment_id: Set(change.comment_id)',
  '.insert(&txn)',
  '.publish_in_tx(',
  'DomainEvent::BlogPostUpdated',
  'txn.commit().await?;',
  'Column::TenantId.eq(tenant_id)',
  'Column::Version.eq(post.version)',
  'next_comment_projection_state(post.comment_count, post.version, delta)',
  'for attempt_index in 0..MAX_PROJECTION_UPDATE_ATTEMPTS',
  'match projection_update_decision(attempt_index, result.rows_affected)',
  'Error::NotFound',
  'impl EventHandler for BlogCommentProjectionHandler',
  'comment_projection_change(event).is_some()',
  '#[cfg(test)]',
  'fn classifies_blog_comment_lifecycle_events()',
  'fn ignores_non_blog_targets_and_unrelated_events()',
  'fn counter_transition_is_non_negative_and_saturating()',
  'fn optimistic_retry_policy_applies_success_without_retry()',
  'fn optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict()',
  'MAX_PROJECTION_UPDATE_ATTEMPTS - 1',
  'Some(&ProjectionUpdateDecision::LimitReached)',
]) {
  requireMarker(handler, marker, handlerPath);
}
requireNoMarker(handler, 'public.blog_posts', handlerPath);

const handlesStart = handler.indexOf('fn handles(&self, event: &DomainEvent) -> bool');
const handleStart = handler.indexOf('async fn handle(&self, envelope: &EventEnvelope)', handlesStart);
if (handlesStart === -1 || handleStart === -1) {
  failures.push(`${handlerPath}: missing EventHandler handles/handle boundary`);
} else {
  const handlesBody = handler.slice(handlesStart, handleStart);
  requireMarker(handlesBody, 'comment_projection_change(event).is_some()', `${handlerPath}: handles`);
  requireNoMarker(handlesBody, 'matches!(', `${handlerPath}: handles`);
}

const retryLoopStart = handler.indexOf(
  'for attempt_index in 0..MAX_PROJECTION_UPDATE_ATTEMPTS',
);
const retryLoopEnd = handler.indexOf('Err(Error::External(format!(', retryLoopStart);
if (retryLoopStart === -1 || retryLoopEnd === -1) {
  failures.push(`${handlerPath}: missing bounded optimistic retry loop boundary`);
} else {
  const retryLoopBody = handler.slice(retryLoopStart, retryLoopEnd);
  requireMarker(
    retryLoopBody,
    'match projection_update_decision(attempt_index, result.rows_affected)',
    `${handlerPath}: retry loop`,
  );
  requireNoMarker(
    retryLoopBody,
    'if result.rows_affected == 1',
    `${handlerPath}: retry loop`,
  );
}

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'const CONCURRENT_PROJECTION_DELIVERIES: usize = 4;',
  'const EXPECTED_RETRY_LIMIT_ATTEMPTS: i64 = 8;',
  'struct PostgresBlogProjectionTestDb',
  'database_url: String',
  'async fn isolated_connection(&self)',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  '.max_connections(1)',
  'SET search_path TO "{schema_name}"',
  'async fn duplicate_delivery_updates_counter_and_outbox_once()',
  'handler.handle(&envelope).await?;',
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
  'running.stop();',
  'async fn wait_for_dispatch_commit(db: &DatabaseConnection, event_id: Uuid)',
  'tokio::time::timeout(Duration::from_secs(5)',
  'count_delivery(db, event_id).await? == 1',
  'event dispatcher did not commit delivery',
  'async fn concurrent_created_events_converge_without_lost_updates()',
  'Arc::new(Barrier::new(envelopes.len()))',
  'let db = test_db.isolated_connection().await?;',
  'tasks.push(tokio::spawn(async move {',
  'barrier.wait().await;',
  'CONCURRENT_PROJECTION_DELIVERIES as i32',
  'count_all_deliveries(&test_db.db).await?',
  'async fn optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears()',
  'install_retry_limit_probe(&test_db.db).await?;',
  'eight zero-row updates must reach the optimistic retry limit',
  'after 8 concurrent attempts',
  'load_retry_attempt_count(&test_db.db).await?',
  'EXPECTED_RETRY_LIMIT_ATTEMPTS',
  'remove_retry_limit_probe(&test_db.db).await?;',
  'CREATE SEQUENCE blog_projection_retry_attempts START WITH 1;',
  'CREATE FUNCTION force_blog_projection_retry_limit()',
  "PERFORM nextval('blog_projection_retry_attempts');",
  'RETURN NULL;',
  'CREATE TRIGGER force_blog_projection_retry_limit',
  'BEFORE UPDATE OF comment_count, version ON blog_posts',
  'DROP TRIGGER force_blog_projection_retry_limit ON blog_posts;',
  'DROP FUNCTION force_blog_projection_retry_limit();',
  'SELECT last_value::bigint AS count FROM blog_projection_retry_attempts',
  'async fn delete_before_create_stays_non_negative_and_replays_in_order()',
  'DomainEvent::CommentDeleted',
  'async fn missing_post_replay_commits_only_after_source_appears()',
  'missing Blog post must keep the delivery retryable',
  'async fn outbox_failure_rolls_back_counter_and_delivery_before_retry()',
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
  'assert!(handler.handles(&blog_deleted));',
  'assert!(!handler.handles(&forum_created));',
]) {
  requireMarker(moduleSource, marker, modulePath);
}
requireNoMarker(moduleSource, 'handler.handle(&', `${modulePath}: host registration harness`);

if (evidence) {
  if (evidence.schema_version !== 4) failures.push(`${evidencePath}: schema_version drift`);
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
  const harnessCases = [...(sourceHarness.cases ?? [])].sort().join('|');
  if (
    harnessCases !== [
      'shared_created_deleted_classifier',
      'non_blog_target_rejection',
      'non_negative_saturating_counter_transition',
      'optimistic_retry_policy_applies_success_without_retry',
      'optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: source harness case drift`);
  }
  const hostRegistration = evidence.host_registration_harness ?? {};
  if (
    hostRegistration.status !== 'executable_no_run' ||
    hostRegistration.runtime_status !== 'not_run' ||
    hostRegistration.path !== modulePath ||
    hostRegistration.module !== 'tests' ||
    hostRegistration.command !== hostRegistrationHarnessCommand ||
    hostRegistration.scope !== 'module_registry_handler_identity_and_routing_only'
  ) {
    failures.push(`${evidencePath}: host registration harness drift`);
  }
  if (
    [...(hostRegistration.cases ?? [])].join('|') !==
    'module_registers_comment_projection_handler_with_host_routing'
  ) {
    failures.push(`${evidencePath}: host registration harness case drift`);
  }
  const dispatcher = evidence.dispatcher_harness ?? {};
  if (
    dispatcher.status !== 'executable_no_run' ||
    dispatcher.runtime_status !== 'not_run' ||
    dispatcher.path !== postgresHarnessPath ||
    dispatcher.environment !== postgresHarnessEnvironment ||
    dispatcher.command !== dispatcherHarnessCommand ||
    dispatcher.isolation !== 'unique_schema_one_connection_pool' ||
    dispatcher.scope !== 'event_bus_dispatcher_module_registered_handler_transactional_commit'
  ) {
    failures.push(`${evidencePath}: dispatcher harness drift`);
  }
  if (
    [...(dispatcher.cases ?? [])].join('|') !==
    'event_dispatcher_routes_registered_handler_and_commits_projection'
  ) {
    failures.push(`${evidencePath}: dispatcher harness case drift`);
  }
  const concurrency = evidence.concurrency_harness ?? {};
  if (
    concurrency.status !== 'executable_no_run' ||
    concurrency.runtime_status !== 'not_run' ||
    concurrency.path !== postgresHarnessPath ||
    concurrency.environment !== postgresHarnessEnvironment ||
    concurrency.command !== concurrencyHarnessCommand ||
    concurrency.isolation !== 'unique_schema_four_independent_connections_barrier' ||
    concurrency.scope !== 'concurrent_unique_envelopes_same_post_final_counter_delivery_outbox'
  ) {
    failures.push(`${evidencePath}: concurrency harness drift`);
  }
  if (
    [...(concurrency.cases ?? [])].join('|') !==
    'concurrent_created_events_converge_without_lost_updates'
  ) {
    failures.push(`${evidencePath}: concurrency harness case drift`);
  }
  const retryLimit = evidence.retry_limit_harness ?? {};
  if (
    retryLimit.status !== 'executable_no_run' ||
    retryLimit.runtime_status !== 'not_run' ||
    retryLimit.path !== postgresHarnessPath ||
    retryLimit.environment !== postgresHarnessEnvironment ||
    retryLimit.command !== retryLimitHarnessCommand ||
    retryLimit.isolation !== 'unique_schema_one_connection_pool_before_update_skip_trigger_nontransactional_attempt_sequence' ||
    retryLimit.scope !== 'real_handler_eight_zero_row_updates_terminal_error_atomic_rollback_and_same_envelope_replay' ||
    retryLimit.non_claim !== 'does_not_measure_natural_postgresql_contention_frequency_or_record_execution'
  ) {
    failures.push(`${evidencePath}: retry-limit harness drift`);
  }
  if (
    [...(retryLimit.cases ?? [])].join('|') !==
    'optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears'
  ) {
    failures.push(`${evidencePath}: retry-limit harness case drift`);
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
  const postgresCases = [...(postgres.cases ?? [])].sort().join('|');
  if (
    postgresCases !== [
      'duplicate_delivery_updates_counter_and_outbox_once',
      'optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears',
      'delete_before_create_stays_non_negative_and_replays_in_order',
      'missing_post_replay_commits_only_after_source_appears',
      'outbox_failure_rolls_back_counter_and_delivery_before_retry',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: PostgreSQL harness case drift`);
  }
  const restart = evidence.restart_harness ?? {};
  if (
    restart.status !== 'executable_no_run' ||
    restart.runtime_status !== 'not_run' ||
    restart.path !== restartHarnessPath ||
    restart.environment !== postgresHarnessEnvironment ||
    restart.command !== restartHarnessCommand ||
    restart.isolation !== 'unique_schema_new_connection' ||
    restart.scope !== 'same_process_new_connection_and_handler'
  ) {
    failures.push(`${evidencePath}: restart harness drift`);
  }
  if (
    [...(restart.cases ?? [])].join('|') !==
    'restarted_handler_reuses_delivery_ledger_without_reapplying_counter'
  ) {
    failures.push(`${evidencePath}: restart harness case drift`);
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
  const processRestartCases = [...(processRestart.cases ?? [])].sort().join('|');
  if (
    processRestartCases !== [
      'restarted_process_reuses_delivery_ledger_without_reapplying_counter',
      'process_restart_worker_applies_envelope_from_env',
    ].sort().join('|')
  ) {
    failures.push(`${evidencePath}: process restart harness case drift`);
  }
  const events = [...(evidence.events ?? [])].sort().join('|');
  if (events !== ['comment.created', 'comment.deleted'].sort().join('|')) {
    failures.push(`${evidencePath}: event set drift`);
  }
  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    'shared_event_classifier',
    'blog_post_target_filter',
    'created_deleted_delta',
    'envelope_idempotency',
    'atomic_counter_delivery_outbox',
    'tenant_scoped_optimistic_update',
    'bounded_optimistic_retry_policy',
    'postgres_retry_limit_rollback_and_replay',
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
}

if (registry) {
  if (registry.schema_version !== 13) failures.push(`${registryPath}: schema_version drift`);
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
  const sourceGate = registry.verification_chain?.source_gates?.comments_event_projection;
  if (sourceGate?.unit_test !== handlerPath) {
    failures.push(`${registryPath}: comments event projection unit test path drift`);
  }
  if (sourceGate?.postgres_test !== postgresHarnessPath) {
    failures.push(`${registryPath}: comments event projection PostgreSQL test path drift`);
  }
  if (sourceGate?.restart_test !== restartHarnessPath) {
    failures.push(`${registryPath}: comments event projection restart test path drift`);
  }
}

for (const marker of [
  'blog-comments-event-projection.json',
  'verify:blog:comments-event-projection',
  'test:verify:blog:comments-event-projection',
  'source_verified_no_compile',
  'services::comment_projection::tests',
  'ProjectionUpdateDecision',
  'seven retry decisions',
  'tests::module_registers_comment_projection_handler_with_host_routing',
  'event_dispatcher_routes_registered_handler_and_commits_projection',
  'concurrent_created_events_converge_without_lost_updates',
  'optimistic_retry_limit_rolls_back_and_replays_after_conflict_clears',
  'eight zero-row',
  'restarted_process_reuses_delivery_ledger_without_reapplying_counter',
  'comment_projection_postgres_test',
  'comment_projection_restart_postgres_test',
  'RUSTOK_BLOG_TEST_DATABASE_URL',
  'EventBus',
  'EventDispatcher',
  'independent PostgreSQL connections',
  'same envelope',
  'two sequential OS test processes',
  'server-host restart',
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error('Blog comments event projection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog comments event projection classifier, retry policy, registration, dispatcher, concurrency, PostgreSQL retry-limit, connection restart, and process restart harnesses are consistent');
