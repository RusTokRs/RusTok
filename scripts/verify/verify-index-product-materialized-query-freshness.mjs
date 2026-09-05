#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-materialized-query-freshness] ${message}`);
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

const admissionPath = 'crates/rustok-index/src/application/postgres_query_admission.rs';
const admission = requireMarkers(admissionPath, [
  'ENTITY_ALIAS_TOKEN: &str = "{{entity}}"',
  'MAX_ENTITY_ADMISSION_BYTES: usize = 32 * 1024',
  'INDEX_ENTITY_ALIAS_MARKER: &str = "index_entities AS \\""',
  'pub struct PostgresQueryEntityAdmission',
  'BindPlaceholderForbidden',
  'MissingEntityRelation',
  'InvalidEntityAlias',
  'EntityAnchorMissing',
  'compiled.exact_count.as_mut()',
  'entity_aliases(sql)?',
  'alias.strip_prefix("mp")',
  '.strip_prefix("mx_t")',
  '.strip_prefix("mo_t")',
  'let anchor = format!("{alias_q}.is_deleted = FALSE")',
  '*sql = sql.replace(&anchor, &replacement)',
]);
forbidMarkers(admissionPath, admission, [
  'PostgresQueryRootAdmission',
  '{{root}}',
  'DatabaseConnection',
  'Statement::',
  'IndexMutation',
  'tokio::spawn',
  'loop {',
]);

const catalogPath = 'crates/rustok-index/src/infrastructure/postgres/query_admission.rs';
const catalog = requireMarkers(catalogPath, [
  'pub struct PostgresIndexQueryAdmissionCatalog',
  'rule: Option<PostgresQueryEntityAdmission>',
  'required_link_targets: BTreeMap<SchemaRef, String>',
  'pub fn register_postgres_index_query_admission(',
  'pub fn register_postgres_index_query_link_target_availability(',
  'pub(crate) fn ensure_runtime_schema(',
  'pub(crate) fn apply_link_target_availability(',
  'fn rebuild_owner_composite(',
  'fn referenced_first_hop_links(query: &IndexQuery)',
  'query.referenced_paths()',
  '{link}.source_version = {root}.source_version',
  '{target}.is_deleted = FALSE',
]);
forbidMarkers(catalogPath, catalog, [
  'PostgresQueryRootAdmission',
  '{{root}}',
  'DatabaseConnection',
  'Statement::',
  'tokio::spawn',
  'rustok-product',
]);

const portPath = 'crates/rustok-index/src/infrastructure/postgres/query_port.rs';
const port = requireMarkers(portPath, [
  'admissions: PostgresIndexQueryAdmissionCatalog',
  'pub fn with_admissions(',
  'let mut compiled = page_query.compiled().clone()',
  '.apply_link_target_availability(query, compiled)',
  'self.admissions.get(&query.schema)',
  '.admission()',
  '.apply(compiled)',
  'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY',
  'verify_persisted_schemas(transaction, query, required_schemas).await',
  'compiled.exact_count.as_ref()',
]);
const availabilityOffset = port.indexOf('.apply_link_target_availability(query, compiled)');
const ownerOffset = port.indexOf('if let Some(descriptor) = self.admissions.get(&query.schema)');
if (availabilityOffset < 0 || ownerOffset < 0 || availabilityOffset >= ownerOffset) {
  fail(`${portPath} must apply link availability before owner entity admission`);
}
forbidMarkers(portPath, port, ['tokio::spawn', 'IndexMutation::']);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_runtime.rs', [
  'let mut admissions = extensions',
  '.get::<PostgresIndexQueryAdmissionCatalog>()',
  'AdmissionSchemaMissing',
  'LinkAvailabilitySchemaMissing',
  'for (schema, owner_module) in admissions.link_availability_iter()',
  'if !admissions.is_empty()',
  'for registered in registry.registry().iter()',
  'admissions.ensure_runtime_schema(registered.schema.reference.clone())?',
  'PostgresIndexQueryPort::with_admissions(',
]);

const productAdmissionPath = 'crates/rustok-distribution/src/product_index/query_admission.rs';
const productAdmission = requireMarkers(productAdmissionPath, [
  'PRODUCT_QUERY_MATERIALIZED_FRESHNESS',
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
  'FROM products owner_product',
  'product_index_graph_projection_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  '{{entity}}.source_version = current_projection.projection_epoch',
  'translation.locale = {{entity}}.locale_key',
  'FROM product_variants owner_variant',
  'owner_variant.index_revision = {{entity}}.source_version',
  'FROM channels owner_channel',
  'owner_channel.index_revision = {{entity}}.source_version',
  'register_postgres_index_query_link_target_availability',
  'PostgresQueryEntityAdmission::new(template)',
  'PRODUCT_SCHEMA_ROUTING_KEY',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'product_variant_schema_ref()',
  'sales_channel_schema_ref()',
  'extensions.contains::<rustok_channel::ChannelRuntimeSelected>()',
  'SchemaVersion::new(2)',
  'SchemaVersion::INITIAL',
]);
forbidMarkers(productAdmissionPath, productAdmission, [
  'SchemaVersion::new(3)',
  'PostgresQueryRootAdmission',
  '{{root}}',
  'index_entities',
  'index_links',
  '$1',
  'IndexMutation',
  'tokio::spawn',
  'loop {',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'query_admission::register(extensions)?;',
  'assert_eq!(admissions.len(), 2)',
  'assert_eq!(admissions.len(), 3)',
  'assert_eq!(admissions.link_availability_len(), 1)',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'PostgresQueryEntityAdmission',
  'PostgresQueryEntityAdmissionApplyError',
  'PostgresQueryEntityAdmissionError',
]);

const freshnessDoc = requireMarkers(
  'crates/rustok-index/docs/m7-product-materialized-query-freshness.md',
  [
    'Status: `source_complete_link_target_availability_equivalence_execution_pending`',
    '`PostgresQueryEntityAdmission`',
    '`mpN_tN`',
    '`mx_tN`',
    '`mo_tN`',
    'ProductVariant',
    'SalesChannel',
    'Query-path-scoped linked-target availability',
    'current link row + missing/stale/deleted target = query fails closed',
    'Recreate monotonicity remains source complete',
    'm20260731_000004_add_product_index_tombstones',
    'm20260731_000011_add_channel_index_tombstones',
    'Filter/order/count/runtime equivalence packet',
    'product_linked_target_recreate_postgres.rs',
    'product_linked_target_availability_equivalence_postgres.rs',
    'Remaining M7 evidence',
  ],
);
forbidMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', freshnessDoc, [
  'Remaining linked-target availability boundary',
  'does **not** claim delete+recreate identity safety',
  'next source slice must make those two owner source clocks monotonic',
  'define and retain fail-closed linked-target availability semantics',
  'retain PostgreSQL cases for linked filtering and many aggregate ordering',
]);

console.log('[verify-index-product-materialized-query-freshness] current Product graph freshness, availability and equivalence source contracts verified');
