#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-runtime-composition] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const neutralPath = 'crates/rustok-index/src/application/query_runtime.rs';
const neutral = requireMarkers(neutralPath, [
  'pub struct SharedIndexQueryRuntime',
  'port: Arc<dyn IndexQueryPort>',
  'pub(crate) fn new(',
  'pub fn shared_port(&self) -> Arc<dyn IndexQueryPort>',
  'impl IndexQueryPort for SharedIndexQueryRuntime',
  'self.port.execute_query(query).await',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresIndexQueryPort',
  'ModuleRuntimeExtensions',
  'SchemaRegistry',
  'index_schemas',
]) {
  if (neutral.includes(forbidden)) {
    fail(`${neutralPath} contains infrastructure/composition marker ${forbidden}`);
  }
}

const materializerPath = 'crates/rustok-index/src/infrastructure/postgres/query_runtime.rs';
const materializer = requireMarkers(materializerPath, [
  'pub enum IndexQueryRuntimeCompositionError',
  'AlreadyMaterialized',
  'AdmissionSchemaMissing',
  'pub fn materialize_postgres_index_query_runtime(',
  'extensions.contains::<SharedIndexQueryRuntime>()',
  'extensions.get::<SharedIndexSchemaRegistry>().cloned()',
  '.get::<PostgresIndexQueryAdmissionCatalog>()',
  '.cloned()',
  '.unwrap_or_default()',
  'for descriptor in admissions.iter()',
  'registry.registry().get(descriptor.schema()).is_none()',
  'descriptor.owner_module().to_owned()',
  'PostgresIndexQueryPort::with_admissions(',
  'registry.shared()',
  'admissions,',
  'extensions.insert(runtime.clone())',
  'return Ok(None);',
  'missing_source_registry_does_not_publish_false_runtime',
  'complete_source_registry_materializes_one_shared_runtime',
  'query_admission_catalog_is_snapshotted_into_runtime_composition',
  'dangling_query_admission_schema_fails_composition',
  'duplicate_query_runtime_materialization_fails_closed',
]);
for (const forbidden of [
  '.query_one(',
  '.query_all(',
  '.execute(',
  '.begin()',
  'index_schemas',
  'social_graph_relation_index_schema',
]) {
  if (materializer.includes(forbidden)) {
    fail(`${materializerPath} performs forbidden startup I/O or imports owner contracts: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod query_runtime;',
  'pub use query_runtime::SharedIndexQueryRuntime;',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod query_runtime;',
  'IndexQueryRuntimeCompositionError',
  'materialize_postgres_index_query_runtime',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'materialize_postgres_index_query_runtime',
  'IndexQueryRuntimeCompositionError',
]);

const serverPath = 'apps/server/src/services/mod.rs';
const server = requireMarkers(serverPath, [
  '#[path = "module_event_dispatcher.rs"]',
  'mod module_event_dispatcher_base;',
  'pub mod module_event_dispatcher {',
  'build_shared_runtime_extensions_with_host_providers(',
  'module_event_dispatcher_base::build_shared_runtime_extensions_with_host_providers(',
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'Index query runtime composition failed',
  'host_materializes_social_graph_index_query_replay_and_event_runtimes',
  'extensions.contains::<SharedIndexSchemaRegistry>()',
  'extensions.contains::<SharedIndexQueryRuntime>()',
  'host.shared_get::<SharedIndexQueryRuntime>().is_some()',
]);
for (const forbidden of ['PostgresIndexQueryPort::new', 'PostgresIndexQueryPort::with_admissions']) {
  if (server.includes(forbidden)) {
    fail(`${serverPath} must call the Index-owned materializer instead of constructing the port`);
  }
}

for (const relative of [
  'crates/rustok-distribution/src/lib.rs',
  'crates/rustok-social-graph/src/lib.rs',
  'crates/rustok-social-graph/src/index_consumer.rs',
  'crates/rustok-social-graph/src/index_privacy.rs',
  'crates/rustok-social-graph/src/index_privacy_shadow.rs',
]) {
  const source = read(relative);
  for (const forbidden of ['PostgresIndexQueryPort::new', 'PostgresIndexQueryPort::with_admissions']) {
    if (source.includes(forbidden)) {
      fail(`${relative} must not construct the Index query port directly`);
    }
  }
}

requireMarkers('xtask/src/server_event_runtime_contracts.rs', [
  'direct_dispatcher_export',
  'dispatcher_facade_export',
  'reviewed base-module facade',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-query-runtime-composition.mjs'",
]);
requireMarkers('crates/rustok-index/CRATE_API.md', [
  '`SharedIndexQueryRuntime`',
  '`materialize_postgres_index_query_runtime`',
  'Runtime presence does not claim',
  'Calling `PostgresIndexQueryPort::new` outside the Index-owned runtime materializer',
]);
requireMarkers('crates/rustok-index/README.md', [
  'M4 source-owned registry and server query-runtime composition: source complete',
  '`SharedIndexQueryRuntime`',
  'Composition performs no SQL',
]);
requireMarkers('crates/rustok-index/docs/m4-query-runtime-composition.md', [
  'Status: `source_complete_execution_pending`',
  '`SharedIndexQueryRuntime`',
  '`materialize_postgres_index_query_runtime(extensions, db)`',
  'performs no SQL',
  'selected consumers may be recomposed only after runtime publication',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 server-owned shared query runtime composition: `source_complete_execution_pending`',
  '`SharedIndexQueryRuntime` is a neutral cloneable `IndexQueryPort` capability',
  'Runtime presence does not establish persisted tenant schema readiness',
]);

console.log('[verify-index-query-runtime-composition] OK');
