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
  'ROOT_ALIAS_TOKEN: &str = "{{root}}"',
  'MAX_ROOT_ADMISSION_BYTES: usize = 32 * 1024',
  'BindPlaceholderForbidden',
  'RootAnchorMismatch',
  'pub fn apply(',
  '.columns',
  '.iter()',
  '.find_map(',
  'CompiledQueryColumn::EntityId',
  'compiled.exact_count.as_mut()',
  'root_baseline(root_alias)',
  '.locale_key = $5',
  'sql.match_indices(&anchor).count() != 1',
  '*sql = sql.replacen(&anchor, &replacement, 1)',
]);
forbidMarkers(admissionPath, admission, [
  'DatabaseConnection',
  'Statement::',
  'IndexMutation',
  'tokio::spawn',
  'loop {',
]);

const catalogPath = 'crates/rustok-index/src/infrastructure/postgres/query_admission.rs';
const catalog = requireMarkers(catalogPath, [
  'pub struct PostgresIndexQueryAdmissionCatalog',
  'BTreeMap<SchemaRef, PostgresIndexQueryAdmissionDescriptor>',
  'pub fn register_postgres_index_query_admission(',
  'DuplicateSchema',
  '.get_or_insert_with::<PostgresIndexQueryAdmissionCatalog',
]);
forbidMarkers(catalogPath, catalog, ['DatabaseConnection', 'Statement::', 'tokio::spawn']);

const portPath = 'crates/rustok-index/src/infrastructure/postgres/query_port.rs';
const port = requireMarkers(portPath, [
  'admissions: PostgresIndexQueryAdmissionCatalog',
  'pub fn with_admissions(',
  'let mut compiled = page_query.compiled().clone()',
  'self.admissions.get(&query.schema)',
  '.admission()',
  '.apply(&mut compiled)',
  'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY',
  'verify_persisted_schemas(transaction, query, required_schemas).await?',
  'compiled_statement(compiled)',
  'compiled.exact_count.as_ref()',
  'self.registry.decode_postgres_query_page(query, page_query, page_rows, exact_count_row)',
]);
forbidMarkers(portPath, port, ['tokio::spawn', 'IndexMutation::']);

const runtimePath = 'crates/rustok-index/src/infrastructure/postgres/query_runtime.rs';
requireMarkers(runtimePath, [
  'PostgresIndexQueryAdmissionCatalog',
  '.get::<PostgresIndexQueryAdmissionCatalog>()',
  '.cloned()',
  '.unwrap_or_default()',
  'AdmissionSchemaMissing',
  'PostgresIndexQueryPort::with_admissions(',
  'query_admission_catalog_is_snapshotted_into_runtime_composition',
  'dangling_query_admission_schema_fails_composition',
]);

const productAdmissionPath = 'crates/rustok-distribution/src/product_index/query_admission.rs';
const productAdmission = requireMarkers(productAdmissionPath, [
  'PRODUCT_QUERY_MATERIALIZED_FRESHNESS',
  'FROM products owner_product',
  'product_index_graph_projection_snapshots',
  'ORDER BY projection.projection_epoch DESC',
  'product_sales_channel_index_relation_freshness_snapshots',
  'witness.relation_epoch = current_projection.relation_epoch',
  'ORDER BY witness.sequence_no DESC',
  '{{root}}.source_version = current_projection.projection_epoch',
  'current_projection.product_source_version = owner_product.index_revision',
  'current_freshness.product_source_version <= owner_product.index_revision',
  'channel_index_identity_generations',
  'current_freshness.channel_identity_generation = COALESCE(',
  'product_sales_channel_index_relation_convergence_requests',
  'request.product_source_version > current_freshness.product_source_version',
  'FROM product_translations translation',
  'translation.locale = {{root}}.locale_key',
  'register_postgres_index_query_admission(',
  'SchemaVersion::new(3)',
]);
forbidMarkers(productAdmissionPath, productAdmission, [
  'index_entities',
  'index_links',
  '$1',
  'channel_visibility',
  'IndexMutation',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'tokio::spawn',
  'loop {',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod query_admission;',
  'query_admission::register(extensions)?;',
  'selected_product_bridge_registers_two_current_schemas_three_factories_and_query_admission',
  'PostgresIndexQueryAdmissionCatalog',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'register_postgres_index_query_admission',
  'PostgresIndexQueryAdmissionCatalog',
  'PostgresIndexQueryAdmissionDescriptor',
]);
requireMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', [
  'Status: `source_complete_postgres_packet_source_ready_execution_pending`',
  'source-read -> mutation-apply',
  'A post-result freshness check is insufficient',
  'Generic root query admission',
  'Product admission evidence',
  '`index_entities.source_version` is Product `projection_epoch`',
  'there is no retained Product visibility convergence request',
  'same predicate is',
  'exact-count SQL',
  'Rejected Product owner data',
  'PostgreSQL evidence packet 1',
  'source-ready, not executed, and not admitted',
]);
requireMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'physically present in `index_entities`',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
requireMarkers(compilerPath, [
  'push_identity_column(',
  '&plan.root_alias,',
  'let mut predicates = base.predicates;',
  'predicates.push(compile_filter(plan, filter, &mut bindings)?);',
  'predicates.push(compile_keyset(plan, cursor, &mut bindings)?);',
  'let pagination = compile_pagination(&plan.pagination, &mut bindings)?;',
  'let exact_count = plan',
  '.then(|| compile_exact_count(plan))',
  'AND {root_alias}.locale_key = {locale} AND {root_alias}.is_deleted = FALSE',
]);

console.log('[verify-index-product-materialized-query-freshness] Product root query freshness fence and packet cursor verified');
