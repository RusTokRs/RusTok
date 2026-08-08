#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-runtime-composition] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runtimePath = 'crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const runtime = requireMarkers(runtimePath, [
  'pub struct SharedIndexReplayRuntime',
  'pub async fn run_interruptible<Check>(',
  'Check: FnMut() -> bool',
  '.run_interruptible(request, should_interrupt)',
  'pub enum IndexReplayRuntimeCompositionError',
  'AlreadyMaterialized',
  'MissingSchemaRegistry',
  'DryRun(#[from] IndexReplayDryRunRuntimeCompositionError)',
  'ReconciliationScheduler(#[from] IndexReconciliationSchedulerCompositionError)',
  'pub fn materialize_postgres_index_replay_runtime(',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  '.get::<SharedIndexSchemaRegistry>()',
  'return Ok(None);',
  'materialize_index_replay_dry_run_runtime(extensions)?;',
  'register_postgres_index_reconciliation_work(extensions)?;',
  'PostgresIndexReplayRunner::new(',
  'extensions.insert(runtime.clone())',
  'missing_source_registry_does_not_publish_false_replay_or_work_runtime',
  'source_registry_without_shared_schema_registry_fails_closed',
  'complete_registries_materialize_replay_and_module_work_registration',
  'duplicate_replay_runtime_materialization_fails_closed',
  'assert!(!extensions.contains::<ModuleWorkRegistrations>());',
  'assert!(extensions.contains::<ModuleWorkRegistrations>());',
]);
for (const forbidden of [
  'tokio::spawn', 'tokio::time::sleep', 'loop {', '.query_one(',
  '.query_all(', '.execute(', '.begin()', 'permissions_for(', 'Permission::', 'StopHandle',
]) {
  if (runtime.includes(forbidden)) fail(`${runtimePath} contains ${forbidden}`);
}
const dryRun = runtime.indexOf('materialize_index_replay_dry_run_runtime(extensions)?;');
const work = runtime.indexOf('register_postgres_index_reconciliation_work(extensions)?;');
const replay = runtime.indexOf('let runtime = SharedIndexReplayRuntime::new(', work);
if (dryRun < 0 || work <= dryRun || replay <= work) {
  fail(`${runtimePath} must publish dry-run, work registration, then replay runtime`);
}

const serverPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const server = requireMarkers(serverPath, [
  'pub struct IndexReplayOperatorContext',
  'pub struct IndexReplayOperatorRuntime',
  'inner: rustok_index::SharedIndexReplayRuntime',
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'pub async fn run_shadow(',
  'context.authorize_for(request.tenant_id())?;',
  'self.shadow.run(request).await.map_err(Into::into)',
  'pub async fn run_interruptible<Check>(',
  '.run_interruptible(request, should_interrupt)',
  'Permission::MODULES_MANAGE',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'materialize_postgres_index_sources(extensions, db.clone())',
  'materialize_index_source_registry(extensions)',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  '.get::<rustok_index::SharedIndexReplayDryRunRuntime>()',
  'IndexReplayOperatorRuntime::new(runtime, shadow)',
  'materialize_index_replay_shadow_transport(',
  'continuation.clone()',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db.clone())?;',
]);
for (const forbidden of ['tokio::spawn', 'tokio::time::sleep', 'loop {', 'StopHandle']) {
  if (server.includes(forbidden)) fail(`${serverPath} contains lifecycle marker ${forbidden}`);
}

requireMarkers('apps/server/src/services/index_replay_shadow_transport.rs', [
  'pub struct IndexReplayShadowTransportRuntime',
  'IndexSourceContinuationScope::from_registry(',
  'self.operator.run_shadow(context, request).await?',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs', [
  'impl ModuleWorkRegistration for IndexReconciliationWorkRegistration',
  'impl ModuleWorkSource for PostgresIndexReconciliationWorkAdapter',
  'impl ModuleWorkHandler for PostgresIndexReconciliationWorkAdapter',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod replay_runtime;',
  'mod source_reconciliation_scheduler;',
  'SharedIndexReplayRuntime',
  'register_postgres_index_reconciliation_work',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'host-published replay and query capabilities',
  'host-owned due reconciliation scheduling through the generic module-work lifecycle',
  'SharedIndexReplayRuntime',
  'register_postgres_index_reconciliation_work',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-runtime-composition.md', [
  'Status: `source_complete_owner_execution_pending`',
  'one module-work registration for due reconciliation execution',
  'publishes no replay runtime, no dry-run runtime, no Shadow transport runtime and no empty Index work registration',
  'The Index materializer performs no SQL',
  'starts the single generic `ModuleWorkScheduler` only when registrations exist',
  '`SharedIndexReplayRuntime::run_interruptible`',
  '`IndexReplayShadowTransportRuntime`',
  'The work registration added here is reconciliation-only',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/m6-reconciliation-host-scheduler.md', [
  'Status: `source_complete_owner_execution_pending`.',
  'The generic host scheduler remains the only polling and lifecycle owner',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-runtime-composition.mjs'",
  "'verify-index-replay-shadow-host-dispatch.mjs'",
  "'verify-index-replay-shadow-graphql-transport.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-replay-runtime-composition] shared replay composition keeps Full durable, Shadow no-write/sealed, and reconciliation under the single generic scheduler boundary');
