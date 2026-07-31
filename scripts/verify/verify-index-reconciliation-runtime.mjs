#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-runtime] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-index/src/infrastructure/mod.rs', [
  'pub mod postgres;',
  'pub mod reconciliation;',
]);

const runtimePath = 'crates/rustok-index/src/infrastructure/reconciliation.rs';
const runtime = requireMarkers(runtimePath, [
  'pub struct SharedIndexReconciliationRuntime',
  'runner: Arc<PostgresIndexReconciliationRunner>',
  'pub async fn run(',
  'pub async fn request_cancel(',
  'pub enum IndexReconciliationRuntimeCompositionError',
  'AlreadyMaterialized',
  'MissingSchemaRegistry',
  'pub fn materialize_postgres_index_reconciliation_runtime(',
  'extensions.contains::<SharedIndexReconciliationRuntime>()',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  'extensions.get::<SharedIndexSchemaRegistry>()',
  'PostgresIndexReconciliationRunner::new(',
  'extensions.insert(runtime.clone());',
  'missing_source_registry_does_not_publish_false_reconciliation_runtime',
  'source_registry_without_shared_schema_registry_fails_closed',
  'complete_registries_materialize_one_shared_reconciliation_runtime',
  'duplicate_reconciliation_runtime_materialization_fails_closed',
]);

for (const forbidden of [
  'tokio::spawn',
  'std::thread::spawn',
  'pub fn database',
  'pub fn db(',
  'pub fn sources(',
  'pub fn schema_registry(',
  'GraphQL',
  'Http',
  'Mcp',
]) {
  if (runtime.includes(forbidden)) {
    fail(`${runtimePath} exposes or starts forbidden capability: ${forbidden}`);
  }
}

const materializerStart = runtime.indexOf(
  'pub fn materialize_postgres_index_reconciliation_runtime(',
);
const testsStart = runtime.indexOf('\n#[cfg(test)]', materializerStart);
if (materializerStart < 0 || testsStart < 0) {
  fail(`${runtimePath} does not contain a bounded materializer block`);
}
const materializer = runtime.slice(materializerStart, testsStart);
for (const forbidden of [
  '.begin().await',
  '.execute(',
  '.query_one(',
  '.query_all(',
  '.run(',
  '.request_cancel(',
]) {
  if (materializer.includes(forbidden)) {
    fail(`materializer performs forbidden runtime/database work: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-reconciliation-runtime-composition.md', [
  'Status: `source_complete_server_guard_and_execution_pending`',
  '`SharedIndexReconciliationRuntime::run`',
  '`SharedIndexReconciliationRuntime::request_cancel`',
  '`materialize_postgres_index_reconciliation_runtime`',
  'publishes no false capability',
  'Materialization performs no database I/O and starts no worker',
  'not yet published by `apps/server`',
  'not yet complete drift repair',
  'The canonical M6 reconciliation and drift-repair item therefore remains open',
  'Execution is maintainer-owned',
]);

console.log('[verify-index-reconciliation-runtime] OK');
