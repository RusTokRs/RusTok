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

const catalogPath = 'crates/rustok-index/src/infrastructure/postgres/query_admission.rs';
requireMarkers(catalogPath, [
  'rule: Option<PostgresQueryEntityAdmission>',
  'pub(crate) fn ensure_runtime_schema(',
  'rule: None',
  'fn rebuild_composite(',
  'filter_map(|descriptor|',
  'schema_guard(schema)',
  'PostgresQueryEntityAdmission::new(format!(',
]);

const runtimePath = 'crates/rustok-index/src/infrastructure/postgres/query_runtime.rs';
requireMarkers(runtimePath, [
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
  'extensions.contains::<rustok_channel::ChannelRuntimeSelected>()',
]);
forbidMarkers(ownerPath, owner, ['index_entities', 'index_links', '$1']);

const variantSource = requireMarkers('crates/rustok-distribution/src/product_variant_index.rs', [
  'v.index_revision',
  'tombstone.source_version AS index_revision',
  'PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2',
]);
const channelSource = requireMarkers('crates/rustok-distribution/src/channel_index.rs', [
  'c.index_revision',
  'tombstone.source_version AS index_revision',
  'SchemaVersion::INITIAL',
]);
for (const [relative, source] of [
  ['crates/rustok-distribution/src/product_variant_index.rs', variantSource],
  ['crates/rustok-distribution/src/channel_index.rs', channelSource],
]) {
  if (!source.includes('index_revision')) fail(`${relative} lost current owner source revision`);
}

requireMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', [
  'Query surfaces fenced against stale target payloads',
  'many-cardinality nested projections',
  'many-cardinality `EXISTS` filters',
  'many-cardinality `MIN`/`MAX` aggregate ordering',
  'does **not** yet prove complete linked-target availability semantics',
  'link exists while its target has not yet been materialized',
  'Explicit remaining recreate boundary',
  'hard delete followed by recreation of the same UUID can reset',
  'next source slice must make those two owner source clocks monotonic',
]);

console.log('[verify-index-linked-target-query-freshness] linked target stale-payload fence and remaining availability gate verified');