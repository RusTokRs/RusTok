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
  'availability_link',
  'availability_target',
  'product_variants owner_variant',
  'channels owner_channel',
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
]);

const catalogPath = 'crates/rustok-index/src/infrastructure/postgres/query_admission.rs';
requireMarkers(catalogPath, [
  'rule: Option<PostgresQueryEntityAdmission>',
  'required_link_targets: BTreeMap<SchemaRef, String>',
  'pub fn require_current_link_targets(',
  'pub(crate) fn apply_link_target_availability(',
  'fn referenced_first_hop_links(query: &IndexQuery)',
  'query.referenced_paths()',
  'path.links().first()',
  '{link}.source_version = {root}.source_version',
  '{link}.link_name IN ({requested_links})',
  '{target}.is_deleted = FALSE',
  'owner_dispatch_for_alias(&owner_rules, AVAILABILITY_TARGET_ALIAS)',
  'scalar_only_query_has_no_referenced_link_targets',
  'linked_query_collects_only_first_hop_link_names',
  'availability_predicate_uses_current_source_link_and_owner_admitted_target',
  'root_availability_predicate_applies_to_page_and_count_anchor_shape',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_runtime.rs', [
  'LinkAvailabilitySchemaMissing',
  'for (schema, owner_module) in admissions.link_availability_iter()',
  'if !admissions.is_empty()',
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
  'register_postgres_index_query_link_target_availability',
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

requireMarkers(
  'crates/rustok-product/src/migrations/m20260731_000004_add_product_index_tombstones.rs',
  [
    'CREATE TABLE product_variant_index_tombstones',
    'OLD.index_revision + 1',
    'rustok_product_variant_seed_index_revision_from_tombstone',
    'NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1)',
    'rustok_product_variant_clear_inserted_index_tombstone',
  ],
);
requireMarkers(
  'crates/rustok-channel/src/migrations/m20260731_000011_add_channel_index_tombstones.rs',
  [
    'CREATE TABLE channel_index_tombstones',
    'OLD.index_revision + 1',
    'rustok_channel_seed_index_revision_from_tombstone',
    'NEW.index_revision := GREATEST(NEW.index_revision, retained_source_version + 1)',
    'rustok_channel_clear_inserted_index_tombstone',
  ],
);

const freshnessDoc = requireMarkers(
  'crates/rustok-index/docs/m7-product-materialized-query-freshness.md',
  [
    'Status: `source_complete_link_target_availability_equivalence_execution_pending`',
    'Query-path-scoped linked-target availability',
    'scalar-only Product queries do not become dependent',
    'current link row + missing/stale/deleted target = query fails closed',
    'Recreate monotonicity remains source complete',
    'Filter/order/count/runtime equivalence packet',
    'product_linked_target_recreate_postgres.rs',
    'product_linked_target_availability_equivalence_postgres.rs',
    'Remaining M7 evidence',
  ],
);
forbidMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', freshnessDoc, [
  'Remaining linked-target availability boundary',
  'next unblocked M7 source-design gap',
  'define and retain fail-closed linked-target availability semantics',
  'next source slice must make those two owner source clocks monotonic',
  'retain PostgreSQL cases for linked filtering and many aggregate ordering',
]);

console.log('[verify-index-linked-target-query-freshness] linked target freshness, recreate monotonicity, availability and equivalence source contracts verified');
