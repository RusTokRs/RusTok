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
  '"dep:rustok-api"',
  'rustok-api = { workspace = true, optional = true }',
  '"dep:rustok-events"',
  'rustok-events = { workspace = true, optional = true }',
  '"dep:rustok-outbox"',
  'rustok-outbox = { workspace = true, optional = true }',
], 'Cargo.toml');

const lib = read('crates/rustok-comments/src/lib.rs');
hasAll(lib, ['pub mod ports;', 'pub use ports::*;', 'mod public_read;'], 'lib.rs');

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
], 'comments owner event publication');
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
  const body = implPorts.slice(idx, implPorts.indexOf('\n    async fn ', idx + 1) === -1 ? implPorts.length : implPorts.indexOf('\n    async fn ', idx + 1));
  if (!body.includes('context.require_policy(PortCallPolicy::write())?')) fail(`${op} does not require shared write policy`);
}
for (const op of port.read_operations) {
  const idx = implPorts.indexOf(`async fn ${op}`);
  if (idx === -1) fail(`ports.rs missing read operation ${op}`);
  const body = implPorts.slice(idx, implPorts.indexOf('\n    async fn ', idx + 1) === -1 ? implPorts.length : implPorts.indexOf('\n    async fn ', idx + 1));
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
if (threadWriteEvidence.module !== 'comments' || threadWriteEvidence.surface !== 'thread_write_invariants' || threadWriteEvidence.owner !== 'rustok-comments') fail('thread write evidence identity drift');
if (threadWriteEvidence.status !== 'executable_no_run' || threadWriteEvidence.compile_policy !== 'not_run_by_request') fail('thread write evidence status drift');
const threadWriteCases = threadWriteEvidence.cases.map(c => c.name).sort().join('|');
if (threadWriteCases !== [
  'bulk_bypass_rejection',
  'exact_active_comment_count',
  'historical_counter_repair',
  'historical_position_repair',
  'serialized_position_allocation',
].sort().join('|')) fail('thread write evidence case matrix drift');

const threadContract = threadWriteEvidence.production_contract ?? {};
for (const [key, expected] of Object.entries({
  position_owner: 'crates/rustok-comments/src/entities/comment.rs',
  counter_owner: 'crates/rustok-comments/src/entities/comment_thread.rs',
  repair_migration: 'crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs',
  migration_registry: 'crates/rustok-comments/src/migrations/mod.rs',
  executable_test: registry.evidence.thread_write_invariants_test,
})) {
  if (threadContract[key] !== expected) fail(`thread write ${key} path drift`);
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
const counterOwner = read(threadContract.counter_owner);
hasAll(counterOwner, [
  'impl ActiveModelBehavior for ActiveModel',
  'update_many()',
  'Column::TenantId.eq(tenant_id)',
  'DeletedAt.is_null()',
  '.count(db)',
  'self.comment_count = Set(count)',
], 'comment thread counter owner');
const repairMigration = read(threadContract.repair_migration);
hasAll(repairMigration, [
  'DatabaseBackend::Postgres',
  'DatabaseBackend::Sqlite',
  'UPDATE comment_threads',
  'ROW_NUMBER() OVER',
  'PARTITION BY thread_id',
  '.unique()',
], 'comment thread repair migration');
const migrationRegistry = read(threadContract.migration_registry);
hasAll(migrationRegistry, [
  'mod m20260723_000008_repair_comment_thread_counters;',
  'Box::new(m20260723_000008_repair_comment_thread_counters::Migration)',
], 'comments migration registry');
const invariantTest = read(threadContract.executable_test);
hasAll(invariantTest, [
  'active_model_hooks_override_stale_positions_and_counts',
  'unique_position_index_rejects_active_model_bypass',
  'stale_thread.comment_count = Set(999)',
], 'thread write invariant test');
const threadWriteVerifier = read(registry.evidence.thread_write_invariants_runner);
hasAll(threadWriteVerifier, [
  'self.position = Set(next_position)',
  'self.comment_count = Set(count)',
  'ROW_NUMBER() OVER',
  'unique_position_index_rejects_active_model_bypass',
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
], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `comments` |', 'crates/rustok-comments/contracts/comments-fba-registry.json', registry.evidence.runtime_order_smoke, '`in_progress` | `boundary_ready`'], 'central registry');

console.log('[verify-comments-fba] comments FBA provider metadata, port semantics, runtime-order evidence, and thread write invariants are consistent');
