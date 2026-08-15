#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-source-schema-registry] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const sourceRegistryPath = 'crates/rustok-index/src/application/source_schema_registry.rs';
const sourceRegistry = requireMarkers(sourceRegistryPath, [
  'pub struct IndexSchemaSourceCatalog',
  'BTreeMap<SchemaRef, IndexSchemaSourceDescriptor>',
  'pub struct IndexSchemaSourceDescriptor',
  'pub struct SharedIndexSchemaRegistry',
  'DuplicateSchemaOwner',
  'SchemaIdentityOwnerConflict',
  'Index schema source catalog is empty',
  'registry.register_batch(',
  'register_index_schema_source(',
  'materialize_index_schema_registry(',
  'return Ok(None);',
  'owner module is invalid',
  'catalog_materializes_cross_source_links_as_one_batch',
  'duplicate_schema_reference_rejects_ambiguous_ownership',
  'schema_identity_owner_is_stable_across_versions',
  'extensions_do_not_materialize_an_empty_registry',
  'fn new(registry: Arc<SchemaRegistry>) -> Self',
]);
for (const forbidden of [
  'pub fn new(registry: Arc<SchemaRegistry>)',
  'PostgresIndexQueryPort',
  'PostgresSchemaRegistrationStore',
  'DatabaseConnection',
  'index_schemas',
  'index_entities',
  'index_links',
]) {
  if (sourceRegistry.includes(forbidden)) {
    fail(`${sourceRegistryPath} contains forbidden runtime/storage marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_schema_registry;',
  'IndexSchemaSourceCatalog',
  'SharedIndexSchemaRegistry',
  'materialize_index_schema_registry',
  'register_index_schema_source',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'fn register_runtime_extensions(',
  'get_or_insert_with::<IndexSchemaSourceCatalog',
]);

const socialGraph = requireMarkers('crates/rustok-social-graph/src/lib.rs', [
  '#[cfg(feature = "index")]',
  '&["index", "outbox"]',
  'social_graph_relation_index_schema()',
  'register_index_schema_source(extensions, self.slug(), schema)',
  'module_publishes_complete_index_source_contracts',
]);
if (socialGraph.includes('PostgresIndexQueryPort::new')) {
  fail('Social Graph module composition must not construct the query port');
}

const distributionPath = 'crates/rustok-distribution/src/lib.rs';
const distribution = requireMarkers(distributionPath, [
  'materialize_index_schema_sources(&mut extensions)?;',
  'fn materialize_index_schema_sources(',
  'materialize_index_schema_registry(extensions)',
  'SharedIndexSchemaRegistry',
  'shared Index schema registry is already materialized',
  'source_schema_catalog_materializes_after_all_modules_register',
  'empty_source_catalog_does_not_publish_false_query_registry',
]);
const productionDistribution = distribution.split('#[cfg(test)]')[0];
for (const forbidden of [
  'social_graph_relation_index_schema',
  'PostgresIndexQueryPort::new',
  'SchemaRegistry::new()',
  'IndexSchema {',
]) {
  if (productionDistribution.includes(forbidden)) {
    fail(`${distributionPath} production composition contains forbidden owner/runtime marker ${forbidden}`);
  }
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-schema-registry.mjs'",
  "'verify-index-source-replay-contract.mjs'",
  "'verify-index-query-runtime-composition.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-source-schema-registry.md', [
  'Status: `source_complete_execution_pending`',
  '`social_graph`',
  'entire schema identity across versions',
  '`materialize_postgres_index_query_runtime`',
  'does not:',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 source-owned immutable schema registry: `source_complete_execution_pending`',
  '`SharedIndexSchemaRegistry`',
  'M4 server-owned shared query runtime composition: `source_complete_execution_pending`',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
]);

console.log('[verify-index-source-schema-registry] OK');