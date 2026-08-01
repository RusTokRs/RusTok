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

const dryRunPath = 'crates/rustok-index/src/replay_dry_run.rs';
const dryRun = requireMarkers(dryRunPath, [
  'pub struct IndexReplayDryRunRequest',
  'MAX_DRY_RUN_PAGES: usize = 1_024',
  'pub enum IndexReplayDryRunStatus',
  'Complete',
  'Yielded',
  'pub struct IndexReplayDryRunOutcome',
  'pub struct SharedIndexReplayDryRunRuntime',
  'pub async fn run(',
  'source_for_schema(request.schema())',
  'IndexSourceScanRequest::new(',
  'let mut event_ids = BTreeSet::new();',
  'event_id.is_nil()',
  'DuplicateEventId',
  'self.schemas.validate_mutation(mutation)',
  'IndexMutation::Upsert',
  'IndexMutation::Delete',
  'next_cursor: cursor',
  'pub fn materialize_index_replay_dry_run_runtime(',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  '.get::<SharedIndexSchemaRegistry>()',
  'extensions.insert(runtime.clone())',
  'bounded_dry_run_yields_a_resume_cursor_and_completes_without_state',
  'dry_run_rejects_a_schema_invalid_mutation_before_any_persistence_boundary',
  'dry_run_request_bounds_page_budget',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'index_entities',
  'index_links',
  'index_inbox',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  '.execute(',
  '.begin()',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
]) {
  if (dryRun.includes(forbidden)) {
    fail(`${dryRunPath} contains forbidden persistence/scheduler marker ${forbidden}`);
  }
}

const indexRuntimePath = 'crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const indexRuntime = requireMarkers(indexRuntimePath, [
  'pub struct SharedIndexReplayRuntime',
  'pub async fn run(',
  'pub async fn request_cancel(',
  'pub enum IndexReplayRuntimeCompositionError',
  'AlreadyMaterialized',
  'MissingSchemaRegistry',
  'DryRun(#[from] IndexReplayDryRunRuntimeCompositionError)',
  'pub fn materialize_postgres_index_replay_runtime(',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  '.get::<SharedIndexSchemaRegistry>()',
  'return Ok(None);',
  'materialize_index_replay_dry_run_runtime(extensions)?;',
  'PostgresIndexReplayRunner::new(',
  'extensions.insert(runtime.clone())',
  'missing_source_registry_does_not_publish_false_replay_runtime',
  'source_registry_without_shared_schema_registry_fails_closed',
  'complete_registries_materialize_replay_and_dry_run_runtimes',
  'extensions.contains::<SharedIndexReplayDryRunRuntime>()',
  'duplicate_replay_runtime_materialization_fails_closed',
]);
for (const forbidden of [
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  '.query_one(',
  '.query_all(',
  '.execute(',
  '.begin()',
  'permissions_for(',
  'Permission::',
]) {
  if (indexRuntime.includes(forbidden)) {
    fail(`${indexRuntimePath} contains host/IO/scheduler marker ${forbidden}`);
  }
}

const serverRuntimePath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const serverRuntime = requireMarkers(serverRuntimePath, [
  'pub struct IndexReplayOperatorContext',
  'pub struct IndexReplayOperatorRuntime',
  'pub enum IndexReplayOperatorError',
  'Permission::MODULES_MANAGE',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'requested_tenant != self.tenant_id',
  'pub async fn run(',
  'pub async fn request_cancel(',
  'materialize_postgres_index_sources(extensions, db.clone())',
  'materialize_index_source_registry(extensions)',
  'materialize_postgres_index_replay_runtime(extensions, db)',
  'extensions.insert(IndexReplayOperatorRuntime::new(runtime))',
  'missing_replay_sources_do_not_publish_false_host_runtime',
  'complete_source_catalog_publishes_guarded_runtime_to_host_context',
  'duplicate_host_replay_materialization_fails_closed',
  'operator_authorization_requires_exact_tenant_actor_and_modules_manage',
  'operator_context_rejects_nil_identity',
]);
for (const forbidden of [
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'StopHandle',
  'runs_background_workers',
  'rustok_product',
  'rustok_content',
  'rustok_flex',
]) {
  if (serverRuntime.includes(forbidden)) {
    fail(`${serverRuntimePath} contains scheduler/source-domain marker ${forbidden}`);
  }
}

const adapterMaterialization = serverRuntime.indexOf(
  'materialize_postgres_index_sources(extensions, db.clone())',
);
const sourceMaterialization = serverRuntime.indexOf(
  'materialize_index_source_registry(extensions)',
  adapterMaterialization,
);
const replayMaterialization = serverRuntime.indexOf(
  'materialize_postgres_index_replay_runtime(extensions, db)',
  sourceMaterialization,
);
const operatorPublication = serverRuntime.indexOf(
  'extensions.insert(IndexReplayOperatorRuntime::new(runtime))',
  replayMaterialization,
);
if (
  adapterMaterialization < 0
  || sourceMaterialization <= adapterMaterialization
  || replayMaterialization <= sourceMaterialization
  || operatorPublication <= replayMaterialization
) {
  fail('server must construct source adapters before registry, replay runtime, and operator publication');
}

const servicesPath = 'apps/server/src/services/mod.rs';
const services = requireMarkers(servicesPath, [
  'pub mod index_replay_runtime_composition;',
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
  'index_replay_runtime_composition::materialize_index_replay_runtime(',
  'canonical Index query and replay runtimes',
]);
const queryRuntime = services.indexOf(
  'materialize_postgres_index_query_runtime(&mut extensions, db.clone())',
);
const replayRuntime = services.indexOf(
  'index_replay_runtime_composition::materialize_index_replay_runtime(',
  queryRuntime,
);
const optionalShadow = services.indexOf('social_graph_index_privacy_shadow_enabled()', replayRuntime);
if (queryRuntime < 0 || replayRuntime <= queryRuntime || optionalShadow <= replayRuntime) {
  fail('server must publish query then replay capabilities before optional Index shadows');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod replay_runtime;',
  'SharedIndexReplayRuntime',
  'IndexReplayRuntimeCompositionError',
  'materialize_postgres_index_replay_runtime',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'bounded side-effect-free',
  'pub mod replay_dry_run;',
  'pub use replay_dry_run::*;',
  'SharedIndexReplayRuntime',
  'IndexReplayRuntimeCompositionError',
  'materialize_postgres_index_replay_runtime',
]);
requireMarkers('crates/rustok-index/docs/m6-bounded-replay-dry-run.md', [
  'Status: `source_complete_host_guard_pending`',
  '`IndexReplayDryRunRequest`',
  '`SharedIndexReplayDryRunRuntime::run`',
  'one invocation budget from 1 through 1024 pages',
  'complete `SchemaRegistry::validate_mutation` validity',
  'No-write boundary',
  'server-owned request-bound `modules:manage` delegation for dry-run invocation',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-runtime-composition.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`IndexReplayOperatorRuntime`',
  '`modules:manage`',
  'Composition performs no SQL and starts no task.',
  'Transport adapters must not retrieve or call `SharedIndexReplayRuntime` directly.',
  'Graceful shutdown and task ownership remain open',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M6 replay runtime host composition and operator guard: `source_complete_owner_execution_pending`',
  '- [x] Bind job requests directly to the materialized source registry in server composition.',
  '`IndexReplayOperatorRuntime` requires an exact request-bound tenant/actor permission snapshot',
  'host scheduling, graceful task shutdown,',
]);
requireMarkers('crates/rustok-index/CRATE_API.md', [
  '`SharedIndexReplayRuntime`',
  '`materialize_postgres_index_replay_runtime`',
  '`IndexReplayOperatorRuntime`',
  'Runtime composition performs no SQL and starts no task.',
]);
requireMarkers('crates/rustok-index/README.md', [
  'M6 replay runtime host composition and operator guard: source complete',
  '`IndexReplayOperatorRuntime`',
  '`modules:manage`',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  'M6 replay runtime host composition and operator guard: `source_complete_owner_execution_pending`',
  '[M6 replay runtime host composition](./m6-replay-runtime-composition.md)',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-runtime-composition.mjs'",
]);

console.log('[verify-index-replay-runtime-composition] OK');
