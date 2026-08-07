#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-link-target-availability] ${message}`);
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

const catalogPath = 'crates/rustok-index/src/infrastructure/postgres/query_admission.rs';
const catalog = requireMarkers(catalogPath, [
  'required_link_targets: BTreeMap<SchemaRef, String>',
  'pub fn link_availability_len(&self) -> usize',
  'pub fn require_current_link_targets(',
  'DuplicateLinkAvailabilitySchema',
  'pub fn register_postgres_index_query_link_target_availability(',
  'pub(crate) fn apply_link_target_availability(',
  'query.referenced_paths()',
  'path.links().first()',
  'if link_names.is_empty()',
  'require_requested_link_targets_predicate(&link_names, &target_owner_admission)',
  'apply_root_predicate(&mut compiled.sql, &predicate)?',
  'compiled.exact_count.as_mut()',
  'availability_link.source_version = {root}.source_version',
  'availability_link.link_name IN ({requested_links})',
  'availability_target.entity_id = availability_link.target_entity_id',
  'availability_target.locale_key = availability_link.target_locale_key',
  'availability_target.is_deleted = FALSE',
  'owner_dispatch_for_alias(&owner_rules, AVAILABILITY_TARGET_ALIAS)',
  'scalar_only_query_does_not_require_unreferenced_link_targets',
  'queried_link_requires_current_owner_admitted_target_in_page_and_count',
]);
forbidMarkers(catalogPath, catalog, [
  'rustok-product',
  'product_variants owner_variant',
  'channels owner_channel',
  'channel_index_identity_generations',
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
]);

const portPath = 'crates/rustok-index/src/infrastructure/postgres/query_port.rs';
const port = requireMarkers(portPath, [
  'let mut compiled = page_query.compiled().clone()',
  '.apply_link_target_availability(query, &mut compiled)',
  'if let Some(descriptor) = self.admissions.get(&query.schema)',
  '.admission()',
  '.apply(&mut compiled)',
  'verify_persisted_schemas(transaction, query, required_schemas).await?',
]);
const availabilityOffset = port.indexOf('.apply_link_target_availability(query, &mut compiled)');
const entityOffset = port.indexOf('if let Some(descriptor) = self.admissions.get(&query.schema)');
if (availabilityOffset < 0 || entityOffset < 0 || availabilityOffset >= entityOffset) {
  fail('query_port.rs must apply link availability before generic entity admission');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_runtime.rs', [
  'LinkAvailabilitySchemaMissing',
  'for (schema, owner_module) in admissions.link_availability_iter()',
  'registry.registry().get(schema).is_none()',
  'if !admissions.is_empty()',
  'admissions.ensure_runtime_schema(registered.schema.reference.clone())?',
  'dangling_link_availability_schema_fails_composition',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'register_postgres_index_query_link_target_availability',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'register_postgres_index_query_link_target_availability',
]);

const ownerPath = 'crates/rustok-distribution/src/product_index/query_admission.rs';
const owner = requireMarkers(ownerPath, [
  'register_postgres_index_query_link_target_availability',
  'let product_schema = product_schema_ref()?',
  'product_schema.clone()',
  '"selected Product Index linked-target availability registration failed: {error}"',
  'PRODUCT_QUERY_MATERIALIZED_FRESHNESS',
  'PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS',
  'SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS',
]);
if ((owner.match(/register_postgres_index_query_link_target_availability\(/g) ?? []).length !== 2) {
  fail('Product query admission must contain one import-use registration call plus the function name only once in code');
}
forbidMarkers(ownerPath, owner, ['index_entities', 'index_links', '$1']);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'assert_eq!(admissions.len(), 2)',
  'assert_eq!(admissions.len(), 3)',
  'assert_eq!(admissions.link_availability_len(), 1)',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const compiler = read(compilerPath);
forbidMarkers(compilerPath, compiler, [
  'availability_link',
  'availability_target',
  'rustok-product',
  'product_variants owner_variant',
  'channels owner_channel',
]);

const freshnessDoc = requireMarkers(
  'crates/rustok-index/docs/m7-product-materialized-query-freshness.md',
  [
    'Status: `source_complete_link_target_availability_execution_pending`',
    'Query-path-scoped linked-target availability',
    'scalar-only Product queries do not become dependent',
    'source identity **and source_version**',
    'current link row + missing/stale/deleted target = query fails closed',
    'Remaining M7 evidence',
  ],
);
forbidMarkers('crates/rustok-index/docs/m7-product-materialized-query-freshness.md', freshnessDoc, [
  'next unblocked M7 source-design gap',
  'define and retain fail-closed linked-target availability semantics',
]);

console.log('[verify-index-link-target-availability] query-scoped fail-closed availability source contract verified');
