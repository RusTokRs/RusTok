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
const serviceExportPath = 'crates/rustok-blog/src/services/mod.rs';
const entityPath = 'crates/rustok-blog/src/entities/blog_comment_projection_delivery.rs';
const migrationPath = 'crates/rustok-blog/src/migrations/m20260716_000001_create_blog_comment_projection_deliveries.rs';
const migrationRegistryPath = 'crates/rustok-blog/src/migrations/mod.rs';
const modulePath = 'crates/rustok-blog/src/lib.rs';
const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';

const handler = read(handlerPath);
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
  'DomainEvent::CommentCreated',
  'if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, 1)',
  'DomainEvent::CommentDeleted',
  'if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, -1)',
  '_ => return Ok(())',
  'let txn = self.db.begin().await?;',
  'blog_comment_projection_delivery::Entity::find_by_id(envelope.id)',
  'update_comment_count_in_tx(&txn, envelope.tenant_id, post_id, delta).await?;',
  'event_id: Set(envelope.id)',
  '.insert(&txn)',
  '.publish_in_tx(',
  'DomainEvent::BlogPostUpdated',
  'txn.commit().await?;',
  'Column::TenantId.eq(tenant_id)',
  'Column::Version.eq(post.version)',
  'post.comment_count.saturating_add(delta).max(0)',
  'if result.rows_affected == 1',
  'Error::NotFound',
  'impl EventHandler for BlogCommentProjectionHandler',
]) {
  requireMarker(handler, marker, handlerPath);
}
requireNoMarker(handler, 'public.blog_posts', handlerPath);

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
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
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
  const events = [...(evidence.events ?? [])].sort().join('|');
  if (events !== ['comment.created', 'comment.deleted'].sort().join('|')) {
    failures.push(`${evidencePath}: event set drift`);
  }
  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    'blog_post_target_filter',
    'created_deleted_delta',
    'envelope_idempotency',
    'atomic_counter_delivery_outbox',
    'tenant_scoped_optimistic_update',
    'missing_post_retry',
    'non_negative_count',
    'module_listener_registration',
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

if (registry) {
  if (registry.evidence?.comments_event_projection !== evidencePath) {
    failures.push(`${registryPath}: comments event projection evidence path drift`);
  }
  const projection = registry.event_projection ?? {};
  if (
    projection.provider !== 'comments' ||
    projection.handler !== 'BlogCommentProjectionHandler' ||
    projection.delivery_ledger !== 'blog_comment_projection_deliveries' ||
    projection.status !== 'implemented_static_only'
  ) {
    failures.push(`${registryPath}: event projection metadata drift`);
  }
}

for (const marker of [
  'blog-comments-event-projection.json',
  'verify:blog:comments-event-projection',
  'test:verify:blog:comments-event-projection',
  'source_verified_no_compile',
  'runtime delivery and recovery',
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error('Blog comments event projection verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Blog comments event projection source contract is consistent');
