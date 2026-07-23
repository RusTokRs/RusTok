import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-comments-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }

const registryPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
const evidencePath = 'crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(registry.evidence.runtime_order_smoke);
const threadWriteEvidence = json(registry.evidence.thread_write_invariants);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'comments' || registry.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.contract_version !== 'comments.thread.v1') fail('contract_version drift');
const port = registry.ports?.[0];
if (!port || port.name !== 'CommentsThreadPort') fail('port name drift');
hasAll(JSON.stringify(port), ['create_comment','get_comment','list_comments_for_target','list_public_comments_for_target','update_comment','set_comment_status','delete_comment'], 'port operations');
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('port context/error drift');

const manifest = read('crates/rustok-comments/rustok-module.toml');
hasAll(manifest, ['[fba.provider]', 'registry = "contracts/comments-fba-registry.json"', 'contract_version = "comments.thread.v1"'], 'manifest');
const cargo = read('crates/rustok-comments/Cargo.toml');
hasAll(cargo, [
  'rustok-api.workspace = true',
  '"dep:rustok-events"',
  'rustok-events = { workspace = true, optional = true }',
  '"dep:rustok-outbox"',
  'rustok-outbox = { workspace = true, optional = true }',
], 'Cargo.toml');
const lib = read('crates/rustok-comments/src/lib.rs');
hasAll(lib, ['pub mod ports;', 'pub use ports::*;', 'mod public_read;'], 'lib.rs');
const dto = read('crates/rustok-comments/src/dto.rs');
hasAll(dto, [
  'use rustok_api::{RichTextDocument, RichTextView};',
  'pub body: RichTextDocument',
  'pub body: Option<RichTextDocument>',
  'pub body: RichTextView',
  'pub body_text: String',
], 'comments richtext DTO');
if (dto.includes('body_format') || dto.includes('content_json')) fail('comments DTO restored a removed richtext compatibility field');
const bodyEntity = read('crates/rustok-comments/src/entities/comment_body.rs');
if (bodyEntity.includes('body_format')) fail('comment body entity restored the removed format selector');
const richtext = read('crates/rustok-comments/src/richtext.rs');
hasAll(richtext, [
  'RichTextProfile::Comment',
  'serialize_comment_body',
  'project_comment_body',
], 'comments richtext owner adapter');
const richtextContract = registry.richtext_contract;
if (
  richtextContract?.write !== 'rustok_api::RichTextDocument'
  || richtextContract?.read !== 'rustok_api::RichTextView'
  || richtextContract?.profile !== 'comment'
  || richtextContract?.format_selector !== false
) fail('comments richtext registry contract drift');

const ports = read('crates/rustok-comments/src/ports.rs');
const providerImpl = 'impl CommentsThreadPort for InProcessCommentsThreadProvider';
hasAll(ports, ['pub trait CommentsThreadPort', providerImpl, 'PortContext', 'PortError', 'TransactionalEventBus', 'CommentsService::with_event_bus'], 'ports.rs');
const publicRead = read('crates/rustok-comments/src/public_read.rs');
hasAll(publicRead, ['CommentStatus::Approved', 'DeletedAt.is_null()', 'list_public_comments_for_target'], 'public comments projection');
const services = read('crates/rustok-comments/src/services.rs');
hasAll(services, [
  'event_bus: Option<TransactionalEventBus>',
  'pub fn with_event_bus',
  'publish_comment_created_in_tx',
  'publish_comment_deleted_in_tx',
  'DomainEvent::CommentCreated',
  'DomainEvent::CommentDeleted',
  '.publish_in_tx(',
  'find_or_create_thread_in_tx',
  'match thread.insert(txn).await',
  'Err(_) => comment_thread::Entity::find()',
], 'comments owner service');
const lifecycleEvents = registry.events ?? [];
if (lifecycleEvents.map(event => event.type).sort().join('|') !== 'comment.created|comment.deleted') fail('comments lifecycle event registry drift');
for (const event of lifecycleEvents) {
  if (event.owner !== 'comments' || event.publication !== 'rustok_outbox::TransactionalEventBus::publish_in_tx' || event.consumer !== 'blog' || event.projection_status !== 'implemented_static_only') fail(`lifecycle event metadata drift for ${event.type}`);
}
const implStart = ports.indexOf(providerImpl);
if (implStart === -1) fail('ports.rs missing in-process provider impl');
const implPorts = ports.slice(implStart);
for (const op of port.write_operations) {
  const idx = implPorts.indexOf(`async fn ${op}`);
  if (idx === -1) fail(`ports.rs missing write operation ${op}`);
  const next = implPorts.indexOf('\n    async fn ', idx + 1);
  const body = implPorts.slice(idx, next === -1 ? implPorts.length : next);
  if (!body.includes('context.require_policy(PortCallPolicy::write())?')) fail(`${op} does not require shared write policy`);
}
for (const op of port.read_operations) {
  const idx = implPorts.indexOf(`async fn ${op}`);
  if (idx === -1) fail(`ports.rs missing read operation ${op}`);
  const next = implPorts.indexOf('\n    async fn ', idx + 1);
  const body = implPorts.slice(idx, next === -1 ? implPorts.length : next);
  if (!body.includes('context.require_policy(PortCallPolicy::read())?')) fail(`${op} does not require shared read policy`);
}

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
const registryCases = registry.contract_tests.cases.map(c => c.operation).sort().join('|');
const evidenceCases = evidence.cases.map(c => c.operation).sort().join('|');
if (registryCases !== evidenceCases) fail('evidence case matrix drift');

if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.runner !== registry.evidence.runtime_order_smoke_runner || runtimeSmoke.status !== 'executable_no_compile' || runtimeSmoke.contract_version !== registry.contract_version) fail('runtime order smoke header drift');
const fallbackProfiles = registry.contract_tests.fallback_smoke.profiles.slice().sort().join('|');
const smokeProfiles = runtimeSmoke.fallback_profiles.slice().sort().join('|');
if (fallbackProfiles !== smokeProfiles) fail('runtime order fallback profile drift');
const fallbackModes = registry.contract_tests.fallback_smoke.degraded_modes.slice().sort().join('|');
const smokeModes = runtimeSmoke.degraded_modes.slice().sort().join('|');
if (fallbackModes !== smokeModes) fail('runtime order degraded mode drift');
for (const c of runtimeSmoke.cases) {
  const body = implPorts.slice(implPorts.indexOf(`async fn ${c.operation}`));
  for (const marker of c.source_order) {
    if (!body.includes(marker)) fail(`${c.operation} runtime order source marker missing ${marker}`);
  }
}

if (registry.evidence.thread_write_invariants !== 'crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json') fail('thread write evidence path drift');
if (registry.evidence.thread_write_invariants_runner !== 'scripts/verify/verify-comments-thread-write-invariants.mjs') fail('thread write verifier path drift');
if (registry.evidence.thread_write_invariants_test !== 'crates/rustok-comments/tests/thread_write_invariants.rs') fail('thread write test path drift');
if (registry.evidence.thread_creation_concurrency_test !== 'crates/rustok-comments/tests/thread_creation_concurrency.rs') fail('thread creation test path drift');
if (threadWriteEvidence.module !== 'comments' || threadWriteEvidence.surface !== 'thread_write_invariants' || threadWriteEvidence.owner !== 'rustok-comments') fail('thread write evidence identity drift');
if (threadWriteEvidence.status !== 'executable_no_run' || threadWriteEvidence.compile_policy !== 'not_run_by_request') fail('thread write evidence status drift');
const threadWriteCases = threadWriteEvidence.cases.map(c => c.name).sort().join('|');
if (threadWriteCases !== [
  'bulk_bypass_rejection',
  'exact_active_comment_count',
  'historical_counter_repair',
  'historical_position_repair',
  'postgres_concurrent_create_delete',
  'postgres_concurrent_first_thread_creation',
  'serialized_position_allocation',
  'status_only_update_preserves_count',
].sort().join('|')) fail('thread write evidence case matrix drift');

const threadContract = threadWriteEvidence.production_contract ?? {};
for (const [key, expected] of Object.entries({
  position_owner: 'crates/rustok-comments/src/entities/comment.rs',
  counter_and_identity_owner: 'crates/rustok-comments/src/entities/comment_thread.rs',
  identity_lock_entity: 'crates/rustok-comments/src/entities/comment_thread_identity_lock.rs',
  counter_repair_migration: 'crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs',
  identity_lock_migration: 'crates/rustok-comments/src/migrations/m20260723_000009_add_comment_thread_identity_locks.rs',
  migration_registry: 'crates/rustok-comments/src/migrations/mod.rs',
  write_invariant_test: registry.evidence.thread_write_invariants_test,
  first_thread_test: registry.evidence.thread_creation_concurrency_test,
  postgres_environment: 'RUSTOK_COMMENTS_TEST_DATABASE_URL',
})) {
  if (threadContract[key] !== expected) fail(`thread write ${key} drift`);
}

const positionOwner = read(threadContract.position_owner);
hasAll(positionOwner, [
  'impl ActiveModelBehavior for ActiveModel',
  'update_many()',
  'Column::TenantId.eq(tenant_id)',
  'order_by_desc(Column::Position)',
  'checked_add(1)',
  'self.position = Set(next_position)',
], 'comment position owner');
const threadOwner = read(threadContract.counter_and_identity_owner);
hasAll(threadOwner, [
  'serialize_thread_identity(db, &self).await?',
  'matches!(&self.comment_count, ActiveValue::Set(_))',
  'OnConflict::columns',
  'identity_lock::Entity::update_many()',
  'comment thread identity {target_type}:{target_id} already belongs to',
  'DeletedAt.is_null()',
  '.count(db)',
  'self.comment_count = Set(count)',
], 'comment thread owner');
const identityEntity = read(threadContract.identity_lock_entity);
hasAll(identityEntity, [
  '#[sea_orm(table_name = "comment_thread_identity_locks")]',
  'pub tenant_id: Uuid',
  'pub target_type: String',
  'pub target_id: Uuid',
], 'comment thread identity entity');
const counterMigration = read(threadContract.counter_repair_migration);
hasAll(counterMigration, [
  'DatabaseBackend::Postgres',
  'DatabaseBackend::Sqlite',
  'UPDATE comment_threads',
  'ROW_NUMBER() OVER',
  'PARTITION BY thread_id',
  '.unique()',
], 'comment thread repair migration');
const identityMigration = read(threadContract.identity_lock_migration);
hasAll(identityMigration, [
  'CommentThreadIdentityLocks::Table',
  'CommentThreadIdentityLocks::TenantId',
  'CommentThreadIdentityLocks::TargetType',
  'CommentThreadIdentityLocks::TargetId',
  'idx_comment_thread_identity_locks_identity',
  '.unique()',
], 'comment thread identity migration');
const migrationRegistry = read(threadContract.migration_registry);
hasAll(migrationRegistry, [
  'mod m20260723_000008_repair_comment_thread_counters;',
  'Box::new(m20260723_000008_repair_comment_thread_counters::Migration)',
  'mod m20260723_000009_add_comment_thread_identity_locks;',
  'Box::new(m20260723_000009_add_comment_thread_identity_locks::Migration)',
], 'comments migration registry');
const writeTest = read(threadContract.write_invariant_test);
hasAll(writeTest, [
  'active_model_hooks_override_stale_positions_and_counts',
  'status_only_thread_update_preserves_comment_count',
  'unique_position_index_rejects_active_model_bypass',
  'postgres_concurrent_creates_and_delete_preserve_thread_invariants',
  'RUSTOK_COMMENTS_TEST_DATABASE_URL',
  'tokio::join!',
  'assert_eq!(positions, vec![1, 2, 3])',
  'assert_eq!(thread.comment_count, active_count as i32)',
], 'thread write invariant test');
const firstThreadTest = read(threadContract.first_thread_test);
hasAll(firstThreadTest, [
  'postgres_concurrent_first_comments_share_one_thread',
  'CommentsService::new(test_db.db_a.clone())',
  'CommentsService::new(test_db.db_b.clone())',
  'tokio::join!',
  'assert_eq!(first.thread_id, second.thread_id)',
  'assert_eq!(threads.len(), 1)',
  'assert_eq!(threads[0].comment_count, 2)',
], 'thread creation concurrency test');
const threadWriteVerifier = read(registry.evidence.thread_write_invariants_runner);
hasAll(threadWriteVerifier, [
  'identity_lock::Entity::update_many()',
  'postgres_concurrent_first_comments_share_one_thread',
  'status_only_thread_update_preserves_comment_count',
  'postgres_concurrent_creates_and_delete_preserve_thread_invariants',
], 'thread write invariant verifier');

const plan = read('crates/rustok-comments/docs/implementation-plan.md');
hasAll(plan, [
  '- FBA status: `boundary_ready`',
  'comments-fba-registry.json',
  'CommentsThreadPort',
  'comments-contract-test-static-matrix.json',
  registry.evidence.runtime_order_smoke,
  registry.evidence.thread_write_invariants,
  'ActiveModelBehavior',
  'UNIQUE(thread_id, position)',
  'RUSTOK_COMMENTS_TEST_DATABASE_URL',
  'concurrent PostgreSQL',
  'identity-lock',
  'thread_creation_concurrency',
], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `comments` |', 'crates/rustok-comments/contracts/comments-fba-registry.json', registry.evidence.runtime_order_smoke, '`in_progress` | `boundary_ready`'], 'central registry');

console.log('[verify-comments-fba] comments FBA provider metadata, runtime-order evidence, transactional thread writes, and first-thread identity serialization are consistent');
