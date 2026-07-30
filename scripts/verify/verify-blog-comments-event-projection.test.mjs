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
  statusDrift = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-comments-projection-'));
  const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-event-projection.json';
  const handlerPath = 'crates/rustok-blog/src/services/comment_projection.rs';
  const serviceExportPath = 'crates/rustok-blog/src/services/mod.rs';
  const entityPath = 'crates/rustok-blog/src/entities/blog_comment_projection_delivery.rs';
  const migrationPath = 'crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs';
  const migrationRegistryPath = 'crates/rustok-blog/src/migrations/mod.rs';
  const modulePath = 'crates/rustok-blog/src/lib.rs';
  const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';

  write(
    root,
    handlerPath,
    `
const BLOG_POST_TARGET_TYPE: &str = "blog_post";
const MAX_PROJECTION_UPDATE_ATTEMPTS: usize = 8;
DomainEvent::CommentCreated if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, 1)
DomainEvent::CommentDeleted if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, -1)
_ => return Ok(())
let txn = self.db.begin().await?;
${missingDeliveryLookup ? '' : 'blog_comment_projection_delivery::Entity::find_by_id(envelope.id)'}
update_comment_count_in_tx(&txn, envelope.tenant_id, post_id, delta).await?;
event_id: Set(envelope.id)
.insert(&txn)
${missingOutbox ? '' : '.publish_in_tx( DomainEvent::BlogPostUpdated'}
txn.commit().await?;
${missingTenantScope ? '' : 'Column::TenantId.eq(tenant_id)'}
Column::Version.eq(post.version)
post.comment_count.saturating_add(delta).max(0)
if result.rows_affected == 1
Error::NotFound
impl EventHandler for BlogCommentProjectionHandler
`,
  );
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
      schema_version: 1,
      module: 'blog',
      surface: 'comments_event_projection',
      status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
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
      cases: [
        { name: 'blog_post_target_filter' },
        { name: 'created_deleted_delta' },
        { name: 'envelope_idempotency' },
        { name: 'atomic_counter_delivery_outbox' },
        { name: 'tenant_scoped_optimistic_update' },
        { name: 'missing_post_retry' },
        { name: 'non_negative_count' },
        { name: 'module_listener_registration' },
      ],
    }),
  );
  write(
    root,
    registryPath,
    JSON.stringify({
      evidence: { comments_event_projection: evidencePath },
      event_projection: {
        provider: 'comments',
        handler: 'BlogCommentProjectionHandler',
        delivery_ledger: 'blog_comment_projection_deliveries',
        status: 'implemented_static_only',
      },
    }),
  );
  write(
    root,
    'crates/rustok-blog/docs/implementation-plan.md',
    'blog-comments-event-projection.json verify:blog:comments-event-projection test:verify:blog:comments-event-projection source_verified_no_compile runtime delivery and recovery',
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
  const root = fixture({ missingTenantScope: true });
  try {
    assert.notEqual(run(root).status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a projection without envelope-id delivery lookup', () => {
  const root = fixture({ missingDeliveryLookup: true });
  try {
    assert.notEqual(run(root).status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a projection without transactional outbox publication', () => {
  const root = fixture({ missingOutbox: true });
  try {
    assert.notEqual(run(root).status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects missing module event-listener registration', () => {
  const root = fixture({ missingRegistration: true });
  try {
    assert.notEqual(run(root).status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects runtime status promotion without execution', () => {
  const root = fixture({ statusDrift: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /status drift/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
