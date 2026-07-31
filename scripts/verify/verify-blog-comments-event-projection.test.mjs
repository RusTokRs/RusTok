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
  missingDeliveryLookup = false,
  missingOutbox = false,
  missingRegistration = false,
  missingSharedClassifier = false,
  directHandlesClassifier = false,
  missingCounterHarness = false,
  missingPostgresHarness = false,
  missingRollbackCase = false,
  missingPostgresRegistration = false,
  statusDrift = false,
  harnessStatusDrift = false,
  postgresStatusDrift = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-projection-'));
  const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json';
  const handlerPath = 'crates/rustok-blog/src/services/comment_projection.rs';
  const postgresHarnessPath = 'crates/rustok-blog/tests/comment_projection_postgres_test.rs';
  const serviceExportPath = 'crates/rustok-blog/src/services/mod.rs';
  const entityPath = 'crates/rustok-blog/src/entities/blog_comment_projection_delivery.rs';
  const migrationPath = 'crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs';
  const migrationRegistryPath = 'crates/rustok-blog/src/migrations/mod.rs';
  const modulePath = 'crates/rustok-blog/src/lib.rs';
  const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
  const harnessCommand = 'cargo test -p rustok-blog --lib services::comment_projection::tests';
  const postgresHarnessCommand = 'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_postgres_test';

  write(
    root,
    handlerPath,
    `
const BLOG_POST_TARGET_TYPE: &str = "blog_post";
const MAX_PROJECTION_UPDATE_ATTEMPTS: usize = 8;
struct CommentProjectionChange
${missingSharedClassifier ? '' : 'fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange>'}
DomainEvent::CommentCreated
delta: 1
DomainEvent::CommentDeleted
delta: -1
fn next_comment_projection_state(comment_count: i32, version: i32, delta: i32)
comment_count.saturating_add(delta).max(0)
version.saturating_add(1)
${missingSharedClassifier ? '' : 'let Some(change) = comment_projection_change(&envelope.event) else'}
let txn = self.db.begin().await?;
${missingDeliveryLookup ? '' : 'blog_comment_projection_delivery::Entity::find_by_id(envelope.id)'}
update_comment_count_in_tx(&txn, envelope.tenant_id, change.post_id, change.delta).await?;
event_id: Set(envelope.id)
comment_id: Set(change.comment_id)
.insert(&txn)
${missingOutbox ? '' : '.publish_in_tx( DomainEvent::BlogPostUpdated'}
txn.commit().await?;
${missingTenantScope ? '' : 'Column::TenantId.eq(tenant_id)'}
Column::Version.eq(post.version)
next_comment_projection_state(post.comment_count, post.version, delta)
if result.rows_affected == 1
Error::NotFound
impl EventHandler for BlogCommentProjectionHandler
fn handles(&self, event: &DomainEvent) -> bool {
  ${directHandlesClassifier ? 'matches!(event, DomainEvent::CommentCreated { .. })' : 'comment_projection_change(event).is_some()'}
}
async fn handle(&self, envelope: &EventEnvelope)
#[cfg(test)]
fn classifies_blog_comment_lifecycle_events()
fn ignores_non_blog_targets_and_unrelated_events()
${missingCounterHarness ? '' : 'fn counter_transition_is_non_negative_and_saturating()'}
`,
  );

  if (!missingPostgresHarness) {
    write(
      root,
      postgresHarnessPath,
      `
const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
struct PostgresBlogProjectionTestDb
CREATE SCHEMA
DROP SCHEMA IF EXISTS
.max_connections(1)
SET search_path TO
async fn duplicate_delivery_updates_counter_and_outbox_once()
handler.handle(&envelope).await?;
async fn delete_before_create_stays_non_negative_and_replays_in_order()
DomainEvent::CommentDeleted
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

  write(root, serviceExportPath, 'pub use comment_projection::BlogCommentProjectionHandler;');
  write(
    root,
    entityPath,
    `
#[sea_orm(table_name = "blog_comment_projection_deliveries")]
#[sea_orm(primary_key, auto_increment = false)]
pub event_id: Uuid
pub tenant_id: Uuid
pub comment_id: Uuid
pub post_id: Uuid
pub delta: i32
`,
  );
  write(
    root,
    migrationPath,
    `
BlogCommentProjectionDeliveries::EventId
.primary_key()
BlogCommentProjectionDeliveries::TenantId
BlogCommentProjectionDeliveries::PostId
idx_blog_comment_projection_deliveries_tenant_post
`,
  );
  write(
    root,
    migrationRegistryPath,
    `
mod m20260716_000001_create_blog_comment_projection_deliveries;
Box::new(m20260716_000001_create_blog_comment_projection_deliveries::Migration)
`,
  );
  write(
    root,
    modulePath,
    missingRegistration
      ? ''
      : `fn register_event_listeners(
registry.register(services::BlogCommentProjectionHandler::new(ctx.db.clone()));`,
  );
  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 3,
      module: 'blog',
      surface: 'comments_event_projection',
      status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: 'pending',
      owner: 'rustok-blog',
      provider: 'rustok-comments',
      events: ['comment.created', 'comment.deleted'],
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
        cases: [
          'shared_created_deleted_classifier',
          'non_blog_target_rejection',
          'non_negative_saturating_counter_transition',
        ],
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
      cases: [
        { name: 'shared_event_classifier' },
        { name: 'blog_post_target_filter' },
        { name: 'created_deleted_delta' },
        { name: 'envelope_idempotency' },
        { name: 'atomic_counter_delivery_outbox' },
        { name: 'tenant_scoped_optimistic_update' },
        { name: 'missing_post_retry' },
        { name: 'non_negative_count' },
        { name: 'postgres_duplicate_delivery' },
        { name: 'postgres_out_of_order_delete_create' },
        { name: 'postgres_missing_post_recovery' },
        { name: 'postgres_outbox_rollback_recovery' },
        { name: 'module_listener_registration' },
      ],
    }),
  );
  write(
    root,
    registryPath,
    JSON.stringify({
      schema_version: 12,
      evidence: { comments_event_projection: evidencePath },
      verification_chain: {
        source_gates: {
          comments_event_projection: {
            unit_test: handlerPath,
            ...(missingPostgresRegistration ? {} : { postgres_test: postgresHarnessPath }),
          },
        },
      },
      event_projection: {
        provider: 'comments',
        handler: 'BlogCommentProjectionHandler',
        delivery_ledger: 'blog_comment_projection_deliveries',
        status: 'implemented_static_only',
        runtime_status: 'pending',
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
      },
    }),
  );
  write(
    root,
    'crates/rustok-blog/docs/implementation-plan.md',
    'blog-comments-event-projection.json verify:blog:comments-event-projection test:verify:blog:comments-event-projection source_verified_no_compile services::comment_projection::tests comment_projection_postgres_test RUSTOK_BLOG_TEST_DATABASE_URL runtime delivery and recovery',
  );

  return root;
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
  expectRejected({ missingTenantScope: true });
});

test('rejects a projection without envelope-id delivery lookup', () => {
  expectRejected({ missingDeliveryLookup: true });
});

test('rejects a projection without transactional outbox publication', () => {
  expectRejected({ missingOutbox: true });
});

test('rejects missing module event-listener registration', () => {
  expectRejected({ missingRegistration: true });
});

test('rejects project without the shared event classifier', () => {
  expectRejected({ missingSharedClassifier: true }, /missing fn comment_projection_change/);
});

test('rejects a separate EventHandler classifier', () => {
  expectRejected({ directHandlesClassifier: true }, /forbidden matches!/);
});

test('rejects a missing counter transition harness', () => {
  expectRejected(
    { missingCounterHarness: true },
    /missing fn counter_transition_is_non_negative_and_saturating/,
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

test('rejects a registry without the PostgreSQL target', () => {
  expectRejected(
    { missingPostgresRegistration: true },
    /PostgreSQL test path drift/,
  );
});

test('rejects runtime status promotion without execution', () => {
  expectRejected({ statusDrift: true }, /status drift/);
});

test('rejects source harness execution promotion without execution', () => {
  expectRejected({ harnessStatusDrift: true }, /source harness drift/);
});

test('rejects PostgreSQL harness execution promotion without execution', () => {
  expectRejected({ postgresStatusDrift: true }, /PostgreSQL harness drift/);
});
