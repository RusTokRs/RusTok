import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const normalize = (value) => value.replace(/\s+/gu, ' ');
const fail = (message) => {
  console.error(`[verify-index-storage-migrations] ${message}`);
  process.exit(1);
};

const lib = read('crates/rustok-index/src/lib.rs');
const migrationModule = read('crates/rustok-index/src/migrations/mod.rs');
const records = read('crates/rustok-index/src/migrations/m20260727_000001_create_index_records.rs');
const delivery = read('crates/rustok-index/src/migrations/m20260727_000002_create_index_delivery_state.rs');
const operations = read('crates/rustok-index/src/migrations/m20260727_000003_create_index_operations.rs');
const recovery = read(
  'crates/rustok-index/src/migrations/m20260803_000004_create_index_reconciliation_recovery.rs',
);
const migrations = normalize(
  [migrationModule, records, delivery, operations, recovery].join('\n'),
);
const tests = read('crates/rustok-index/src/contract_tests.rs');
const plan = read('crates/rustok-index/docs/implementation-plan.md');
const crateReadme = read('crates/rustok-index/README.md');
const moduleDocs = read('crates/rustok-index/docs/README.md');
const databaseDocs = read('docs/architecture/database.md');

for (const marker of [
  'pub mod migrations;',
  'migrations::migrations()',
  'migrations::migration_dependencies()',
]) {
  if (!lib.includes(marker)) fail(`IndexModule migration source missing ${marker}`);
}

for (const marker of [
  'mod m20260727_000001_create_index_records;',
  'mod m20260727_000002_create_index_delivery_state;',
  'mod m20260727_000003_create_index_operations;',
  'mod m20260803_000004_create_index_reconciliation_recovery;',
  'm20260727_000001_create_index_records::Migration',
  'm20260727_000002_create_index_delivery_state::Migration',
  'm20260727_000003_create_index_operations::Migration',
  'm20260803_000004_create_index_reconciliation_recovery::Migration',
  'm20250101_000001_create_tenants',
  'vec!["m20260727_000003_create_index_operations"]',
]) {
  if (!migrationModule.includes(marker)) fail(`migration registry missing ${marker}`);
}

for (const marker of [
  'IndexSchemas::Table',
  'IndexEntities::Table',
  'IndexLinks::Table',
  'IndexInbox::Table',
  'IndexCheckpoints::Table',
  'IndexJobs::Table',
  'IndexConsistencyFindings::Table',
  'CREATE TABLE index_reconciliation_recovery_audits',
]) {
  if (!migrations.includes(marker)) fail(`canonical storage table missing ${marker}`);
}

if (migrations.includes('.if_not_exists()')) {
  fail('canonical zero-state migrations must fail closed instead of accepting pre-existing drift');
}
for (const marker of [
  'DbBackend::Sqlite => { column.big_integer(); }',
  'DbBackend::Postgres | DbBackend::MySql => { column.decimal_len(20, 0); }',
]) {
  if (!migrations.includes(marker)) fail(`source-version backend contract missing ${marker}`);
}
if ((migrations.match(/super::source_version_column\(/gu) ?? []).length !== 4) {
  fail('entity, link, inbox, and checkpoint source versions must share the backend-aware u64 column contract');
}
for (const forbidden of [
  'Product',
  'Variant',
  'SalesChannel',
  'idx_bench_',
  'TypedEav',
  'HotProjection',
]) {
  if (migrations.includes(forbidden)) fail(`production migration contains source/benchmark marker ${forbidden}`);
}

for (const marker of [
  '.name("pk_index_schemas") .col(IndexSchemas::TenantId) .col(IndexSchemas::ModuleName) .col(IndexSchemas::EntityName) .col(IndexSchemas::SchemaVersion)',
  '.name("pk_index_entities") .col(IndexEntities::TenantId) .col(IndexEntities::ModuleName) .col(IndexEntities::EntityName) .col(IndexEntities::SchemaVersion) .col(IndexEntities::EntityId) .col(IndexEntities::LocaleKey)',
  '.name("uq_index_schemas_fingerprint")',
  '.name("fk_index_entities_schema")',
  '.from_col(IndexEntities::SchemaFingerprint)',
  '.to_col(IndexSchemas::SchemaFingerprint)',
  'source_version >= 0',
  '(is_deleted = FALSE AND payload IS NOT NULL) OR (is_deleted = TRUE AND payload IS NULL)',
  '.string_len(32) .not_null() .default("")',
]) {
  if (!migrations.includes(marker)) fail(`entity storage contract missing ${marker}`);
}

for (const marker of [
  '.name("pk_index_links")',
  '.col(IndexLinks::SourceVersion)',
  '.name("fk_index_links_source_version")',
  '.to_col(IndexEntities::SourceVersion)',
  'ordinal >= 0',
  '.name("idx_index_links_target")',
]) {
  if (!migrations.includes(marker)) fail(`link storage contract missing ${marker}`);
}
if (migrations.includes('fk_index_links_target')) {
  fail('target links must not require target-row arrival before source mutation application');
}

for (const marker of [
  "mutation_kind IN ('upsert', 'delete')",
  "state IN ('pending', 'processing', 'applied', 'rejected')",
  "state = 'processing' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL",
  '.name("pk_index_inbox") .col(IndexInbox::TenantId) .col(IndexInbox::SourceName) .col(IndexInbox::DeliveryId)',
  '.name("idx_index_inbox_claim")',
  "checkpoint_kind IN ('ingestion', 'rebuild')",
  '.name("pk_index_checkpoints")',
]) {
  if (!migrations.includes(marker)) fail(`delivery/checkpoint contract missing ${marker}`);
}

for (const marker of [
  "kind IN ('schema_apply', 'secondary_index', 'rebuild', 'reconcile', 'consistency_check')",
  "state IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')",
  "state = 'running' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL",
  "scope_kind = 'global'",
  "scope_kind = 'schema'",
  "scope_kind = 'entity'",
  '.name("idx_index_jobs_claim")',
  '.name("uq_index_consistency_finding_key")',
  "severity IN ('info', 'warning', 'error')",
  "state IN ('open', 'resolved', 'ignored')",
]) {
  if (!migrations.includes(marker)) fail(`job/consistency contract missing ${marker}`);
}

for (const marker of [
  'ADD COLUMN retry_epoch INTEGER NOT NULL DEFAULT 0',
  'CHECK (retry_epoch >= 0)',
  'tenant_id UUID NOT NULL',
  'audit_id UUID NOT NULL',
  'job_id UUID NOT NULL',
  'actor_id UUID NOT NULL',
  "CHECK (action = 'requeue')",
  'reason VARCHAR(512) NOT NULL',
  'prior_attempt_count INTEGER NOT NULL',
  'UNIQUE (tenant_id, job_id, retry_epoch)',
  'index_reconciliation_recovery_audits_immutable_update',
  'index_reconciliation_recovery_audits_immutable_delete',
  'Index reconciliation recovery audits are append-only',
]) {
  if (!recovery.includes(marker)) fail(`reconciliation recovery migration missing ${marker}`);
}
for (const forbidden of ['ON DELETE CASCADE', 'ON UPDATE CASCADE']) {
  if (recovery.includes(forbidden)) {
    fail(`append-only reconciliation recovery audit must not contain ${forbidden}`);
  }
}

for (const marker of [
  'index_module_registers_canonical_storage_migrations',
  'canonical_storage_migrations_round_trip_on_sqlite',
  'm20260803_000004_create_index_reconciliation_recovery',
  'index_reconciliation_recovery_audits',
  'recovery audit rows must reject updates',
  'recovery audit rows must reject deletes',
  'source-version changes must not strand links on an older entity version',
  'processing deliveries require a complete lease',
  'down migrations must remove all Index tables',
]) {
  if (!tests.includes(marker)) fail(`migration fixture missing ${marker}`);
}

for (const marker of [
  '- [x] Add canonical schema/entity/link/inbox/job/checkpoint/consistency migrations.',
  '- [x] Add tenant/schema/entity/locale keys and source-version guards.',
  'M3 storage-schema foundation: `complete`',
]) {
  if (!plan.includes(marker)) fail(`implementation plan marker missing ${marker}`);
}
for (const document of [crateReadme, moduleDocs, databaseDocs]) {
  for (const table of [
    '`index_schemas`',
    '`index_entities`',
    '`index_links`',
    '`index_inbox`',
    '`index_jobs`',
    '`index_checkpoints`',
    '`index_consistency_findings`',
  ]) {
    if (!document.includes(table)) fail(`storage documentation missing ${table}`);
  }
}
if (!moduleDocs.includes('[M6 Reconciliation Dead-letter Requeue]')) {
  fail('Index architecture docs must link the reconciliation recovery contract');
}

console.log('[verify-index-storage-migrations] ok');
