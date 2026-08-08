#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-graphql-transport] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const transportPath = 'apps/server/src/graphql/index_replay.rs';
const transport = requireMarkers(transportPath, [
  'pub struct IndexReplayRunInput',
  'pub module_name: String',
  'pub entity_name: String',
  'pub schema_version: String',
  'pub struct IndexReplayCancelInput',
  'pub job_id: String',
  'const GRAPHQL_REPLAY_PAGE_LIMIT: usize = 100;',
  'const GRAPHQL_REPLAY_MAX_PAGES: usize = 8;',
  'const GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES: usize = 1;',
  'const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;',
  'async fn run_index_replay(',
  'async fn cancel_index_replay(',
  'prepare_authorized_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_cancel(tenant.id, auth.user_id, input)',
  'permissions_for(&tenant_id, &actor_id)',
  'has_effective_permission(&permissions, &Permission::MODULES_MANAGE)',
  'let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;',
  'let worker_id = format!("graphql-replay-{}", Uuid::new_v4().simple());',
  '.get::<IndexReplayOperatorRuntime>()',
  'let stop_handle = ctx.data::<StopHandle>()?.clone();',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  '.request_cancel(operator_context, job_id)',
  'replay_transport_authorizes_before_parsing_untrusted_run_input',
  'replay_transport_derives_authority_worker_and_server_owned_budgets',
  'replay_cancel_authorizes_before_job_id_parsing_and_derives_tenant',
]);

const authorizeStart = transport.indexOf('fn authorize(');
const permissionCheck = transport.indexOf(
  'let permissions = permissions_for(&tenant_id, &actor_id)',
  authorizeStart,
);
const runPrepare = transport.indexOf('fn prepare_authorized_run(');
const runAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', runPrepare);
const runParse = transport.indexOf(
  'let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;',
  runPrepare,
);
if (
  authorizeStart < 0 ||
  permissionCheck < authorizeStart ||
  runPrepare < 0 ||
  runAuthorize < runPrepare ||
  runParse <= runAuthorize
) {
  fail('run transport must authorize before parsing untrusted schema input');
}

const cancelPrepare = transport.indexOf('fn prepare_authorized_cancel(');
const cancelAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', cancelPrepare);
const cancelParse = transport.indexOf('Uuid::parse_str(bounded_text("job_id"', cancelPrepare);
if (cancelPrepare < 0 || cancelAuthorize < cancelPrepare || cancelParse <= cancelAuthorize) {
  fail('cancel transport must authorize before parsing untrusted job id');
}

const runInputStart = transport.indexOf('pub struct IndexReplayRunInput');
const runInputEnd = transport.indexOf('\n}', runInputStart);
const runInput = transport.slice(runInputStart, runInputEnd);
for (const forbidden of [
  'tenant',
  'actor',
  'user_id',
  'worker',
  'page_limit',
  'max_pages',
  'heartbeat',
  'lease',
  'locale',
  'partition',
  'source_name',
  'StopHandle',
  'is_stopping',
  'Uuid',
]) {
  if (runInput.includes(forbidden)) fail(`replay run input contains caller-owned field marker ${forbidden}`);
}

const production = transport.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'tokio::spawn',
  '.scan(',
  'PostgresIndexReplayRunner',
  'SharedIndexReplayRuntime',
  'PostgresMutationStore',
  'ModuleWorkScheduler',
  '.stop()',
]) {
  if (production.includes(forbidden)) {
    fail(`${transportPath} bypasses the guarded replay/lifecycle boundary: ${forbidden}`);
  }
}

requireMarkers('apps/server/src/graphql/mod.rs', ['pub mod index_replay;']);
requireMarkers('apps/server/src/graphql/schema.rs', [
  'use super::index_replay::IndexReplayMutation;',
  'IndexDriftSourcePageDiagnosisMutation,\n    IndexReplayMutation,',
  'pub stop_handle: StopHandle,',
  '.data(stop_handle)',
]);
requireMarkers('apps/server/src/services/graphql_schema.rs', [
  'let stop_handle = stop_handle_from_context(ctx);',
  'let (candidate, _initial_receiver) = StopHandle::new();',
  'ctx.shared_insert_if_absent(candidate);',
  'IndexReplayStopKeepalive',
  '_receiver: handle.subscribe()',
  'avoid a zero-receiver window',
  'stop_handle,',
]);
requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub struct IndexReplayOperatorRuntime',
  'pub async fn run_interruptible<Check>(',
  '.run_interruptible(request, should_interrupt)',
  'pub async fn request_cancel(',
  'Permission::MODULES_MANAGE',
  'context.authorize_for(request.page_request().tenant_id())?;',
]);
requireMarkers('apps/server/src/services/app_lifecycle.rs', [
  'pub struct StopHandle',
  'pub fn subscribe(&self)',
  'pub async fn stop(&self)',
  'pub fn is_stopping(&self) -> bool',
]);
requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'Status: `source_complete_shutdown_bound_execution_pending`.',
  '`runIndexReplay(input: ...)`',
  '`cancelIndexReplay(input: ...)`',
  'Tenant and actor identities are never accepted',
  'page limit: `100` mutations',
  'maximum pages: `8`',
  'lease duration: `60` seconds',
  '`StopHandle::is_stopping`',
  'delegation only through `IndexReplayOperatorRuntime`',
  'maintainer-owned',
]);

console.log('[verify-index-replay-graphql-transport] guarded schema-wide replay run is bound to the server-owned StopHandle probe; cancel and caller input remain independent of shutdown control');
