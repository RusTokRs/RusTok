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
const postgresHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test';
const restartHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_restart_postgres_test';
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
  'fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>',
  'DomainEvent::CommentCreated',
  'delta: 1',
  'DomainEvent::CommentDeleted',
  'delta: -1',
  'fn next_comment_projection_state(comment_count: i32, version: i32, delta: i32)',
  'comment_count.saturating_add(delta).max(0)',
  'version.saturating_add(1)',
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
  'if result.rows_affected == 1',
  'Error::NotFound',
  'impl EventHandler for BlogCommentProjectionHandler',
  'comment_projection_change(event).is_some()',
  '#[cfg(test)]',
  'fn classifies_blog_comment_lifecycle_events()',
  'fn ignores_non_blog_targets_and_unrelated_events()',
  'fn counter_transition_is_non_negative_and_saturating()',
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

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'struct PostgresBlogProjectionTestDb',
  'CREATE SCHEMA',
  'DROP SCHEMA IF EXISTS',
  '.max_connections(1)',
  'SET search_path TO',
  'async fn duplicate_delivery_updates_counter_and_outbox_once()',
  'handler.handle(&envelope).await?;',
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

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'struct PostgresBlogProjectionRestartTestDb',
  'database_url: String',
  'async fn restarted_connection(&self)',
  'SET search_path TO',
  'async fn restarted_handler_reuses_delivery_ledger_without_reapplying_counter()',
  'let first_handler = BlogCommentProjectionHandler::new(test_db.db.clone());',
  'first_handler.handle(&envelope).await?;',
  'drop(first_handler);',
  'let restarted_db = test_db.restarted_connection().await?;',
  'let restarted_handler = BlogCommentProjectionHandler::new(restarted_db.clone());',
  'restarted_handler.handle(&envelope).await?;',
  'load_post_state(&restarted_db, tenant_id, post_id).await?, (1, 2)',
  'count_delivery(&restarted_db, envelope.id).await?, 1',
  'count_outbox_events(&restarted_db).await?, 1',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
]) {
  requireMarker(restartHarness, marker, restartHarnessPath);
}
requireNoMarker(restartHarness, '#[ignore]', restartHarnessPath);
requireNoMarker(restartHarness, 'runtime_verified', restartHarnessPath);

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
]) {
  requireMarker(moduleSource, marker, modulePath);
}

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
  const postgresCases = [...(postgres.cases ?? [])].sort().join('|');
  if (
    postgresCases !== [
      'duplicate_delivery_updates_counter_and_outbox_once',
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
    restart.isolation !== 'unique_schema_new_connection'
  ) {
    failures.push(`${evidencePath}: restart harness drift`);
  }
  if (
    [...(restart.cases ?? [])].join('|') !==
    'restarted_handler_reuses_delivery_ledger_without_reapplying_counter'
  ) {
    failures.push(`${evidencePath}: restart harness case drift`);
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
    'missing_post_retry',
    'non_negative_count',
    'postgres_duplicate_delivery',
    'postgres_out_of_order_delete_create',
    'postgres_missing_post_recovery',
    'postgres_outbox_rollback_recovery',
    'postgres_restart_replay',
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
  'comment_projection_postgres_test',
  'comment_projection_restart_postgres_test',
  'RUSTOK_BLOG_TEST_DATABASE_URL',
  'runtime delivery and recovery',
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error('Blog comments event projection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog comments event projection source contract, unit harness, PostgreSQL target, and restart target are consistent');
