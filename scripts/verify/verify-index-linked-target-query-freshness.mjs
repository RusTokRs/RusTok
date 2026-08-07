#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-linked-target-query-freshness] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const entityAdmissionPath = 'crates/rustok-index/src/application/postgres_query_admission.rs';
const entityAdmission = requireMarkers(entityAdmissionPath, [
  'index_entities AS \\"',
  'BTreeSet<String>',
  'numeric_suffix(alias, "t")',
  'remainder.split_once("_t")',
  '.strip_prefix("mx_t")',
  '.strip_prefix("mo_t")',
  'is_deleted = FALSE',
  'sql.replace(&anchor, &replacement)',
  'compiled.exact_count.as_mut()',
]);
forbidMarkers(entityAdmissionPath, entityAdmission, [
  'rustok-product',
  'product_variants',
  'channels owner_channel',
  'channel_index_identity_generations',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const compiler = requireMarkers(compilerPath, [
  'FROM index_entities AS {root_alias}',
  'LEFT JOIN index_entities AS {target_alias}',
  'let target_alias_name = format!("mp{projection_index}_t{}", index + 1)',
  'let target_alias_name = format!("mx_t{}", index + 1)',
  'let target_alias_name = format!("mo_t{}", index + 1)',
  'JOIN index_entities AS {target_alias} ON {target_predicate}',
  'compile_many_exists(plan, field, bindings',
  'compile_many_order_aggregate(plan, &order.field, aggregate, bindings)',
  '.then(|| compile_exact_count(plan))',
]);
forbidMarkers(compilerPath, compiler, [
  'product_variants owner_variant',
  'channels owner_channel',
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_admission.rs', [
  'rule: Option<PostgresQueryEntityAdmission>',
  'pub(crate) fn ensure_runtime_schema(',
  'rule: None',
  'fn rebuild_composite(',
  'filter_map(|descriptor|',
  'schema_guard(schema)',
  'PostgresQueryEntityAdmission::new(format!(',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_runtime.rs', [
  'if !admissions.is_empty()',
  'for registered in registry.registry().iter()',
  'admissions.ensure_runtime_schema(registered.schema.reference.clone())?',
  'PostgresIndexQueryPort::with_admissions(',
]);

const ownerPath = 'crates/rustok-distribution/src/product_index/query_admission.rs';
const owner = requireMarkers(ownerPath, [
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'FROM product_variants owner_variant',
  'owner_variant.tenant_id = {{entity}}.tenant_id',
  'owner_variant.id = {{entity}}.entity_id',
  'owner_variant.index_revision = {{entity}}.source_version',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
  'FROM channels owner_channel',
  'owner_channel.tenant_id = {{entity}}.tenant_id',
  'owner_channel.id = {{entity}}.entity_id',
  'owner_channel.index_revision = {{entity}}.source_version',
]);
forbidMarkers(ownerPath, owner, ['index_entities', 'index_links', '$1']);

requireMarkers('crates/rustok-distribution/src/product_variant_index.rs', [
  'v.index_revision',
  'tombstone.source_version AS index_revision',
  'PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2',
]);
requireMarkers('crates/rustok-distribution/src/channel_index.rs', [
  'c.index_revision',
  'tombstone.source_version AS index_revision',
  'SchemaVersion::INITIAL',
]);

const variantTombstoneMigration =
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs';
requireMarkers(variantTombstoneMigration, [
  'CREATE TABLE product_variant_index_tombstones',
  'OLD.index_revision + 1',
  'rustok_product_variant_seed_index_revision_from_tombstone',
  'NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1)',
  'rustok_product_variant_clear_inserted_index_tombstone',
  'tombstone.source_version >= live_source_version',
]);

const channelTombstoneMigration =
  'crates/rustok-channel/src/migrations/m20260731_000011_add_channel_index_tombstones.rs';
requireMarkers(channelTombstoneMigration, [
  'CREATE TABLE channel_index_tombstones',
  'OLD.index_revision + 1',
  'rustok_channel_seed_index_revision_from_tombstone',
  'NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1)',
  'rustok_channel_clear_inserted_index_tombstone',
  'tombstone.source_version >= live_source_version',
]);

const freshnessDoc = requireMarkers(
  'crates/rustok-index/docs/m7-product-materialized-query-freshness.md',
  [
    'Recreate monotonicity is already source complete',
    'do **not** need a new recreate clock',
    'm20260731_000004_add_product_index_tombstones',
    'm20260731_000011_add_channel_index_tombstones',
    'No new ProductVariant/SalesChannel ledger or schema version should be added',
    'Remaining linked-target availability boundary',
    'link exists but the target has not yet been materialized',
    'product_linked_target_recreate_postgres.rs',
  ],
);
forbidMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', freshnessDoc, [
  'next source slice must make those two owner source clocks monotonic',
  'implement recreate-safe monotonic ProductVariant and SalesChannel source clocks',
]);

console.log('[verify-index-linked-target-query-freshness] linked target freshness and retained recreate monotonicity verified');
