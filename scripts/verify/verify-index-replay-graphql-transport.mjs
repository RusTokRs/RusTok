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
  'pub locale: Option<String>',
  'pub struct IndexReplayShadowRunInput',
  'pub continuation: Option<String>',
  'pub struct IndexReplayCancelInput',
  'pub job_id: String',
  'const MAX_LOCALE_BYTES: usize = 32;',
  'const MAX_CONTINUATION_BYTES: usize = 16 * 1024;',
  'const GRAPHQL_REPLAY_PAGE_LIMIT: usize = 100;',
  'const GRAPHQL_REPLAY_MAX_PAGES: usize = 8;',
  'const GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES: usize = 1;',
  'const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;',
  'async fn run_index_replay(',
  'async fn run_index_replay_shadow(',
  'async fn cancel_index_replay(',
  'prepare_authorized_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_shadow_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_cancel(tenant.id, auth.user_id, input)',
  'permissions_for(&tenant_id, &actor_id)',
  'has_effective_permission(&permissions, &Permission::MODULES_MANAGE)',
  'let locale = parse_locale(input.locale)?;',
  'rustok_index::LocaleKey::new(locale)',
  'rustok_index::IndexReplayRunRequest::for_locale(',
  'rustok_index::IndexReplayRunRequest::new(',
  'let worker_id = format!("graphql-replay-{}", Uuid::new_v4().simple());',
  '.get::<IndexReplayOperatorRuntime>()',
  '.get::<IndexReplayShadowTransportRuntime>()',
  'let stop_handle = ctx.data::<StopHandle>()?.clone();',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  '.run_schema_wide(',
  '.request_cancel(operator_context, job_id)',
  'replay_transport_authorizes_before_parsing_untrusted_run_input',
  'shadow_transport_authorizes_before_schema_and_continuation_parsing',
  'shadow_transport_accepts_only_schema_and_bounded_sealed_continuation',
  'replay_transport_derives_authority_worker_and_server_owned_budgets',
  'replay_transport_canonicalizes_optional_locale_after_authorization',
  'replay_cancel_authorizes_before_job_id_parsing_and_derives_tenant',
]);

const runPrepare = transport.indexOf('fn prepare_authorized_run(');
const runAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', runPrepare);
const runSchemaParse = transport.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', runPrepare);
const runLocaleParse = transport.indexOf('let locale = parse_locale(input.locale)?;', runPrepare);
if (runPrepare < 0 || runAuthorize < runPrepare || runSchemaParse <= runAuthorize || runLocaleParse <= runAuthorize) {
  fail('durable Full transport must authorize before parsing untrusted schema/locale input');
}

const shadowPrepare = transport.indexOf('fn prepare_authorized_shadow_run(');
const shadowAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', shadowPrepare);
const shadowSchemaParse = transport.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', shadowPrepare);
const shadowContinuationParse = transport.indexOf('bounded_text("continuation", &value, MAX_CONTINUATION_BYTES)?;', shadowPrepare);
if (shadowPrepare < 0 || shadowAuthorize < shadowPrepare || shadowSchemaParse <= shadowAuthorize || shadowContinuationParse <= shadowSchemaParse) {
  fail('Shadow transport must authorize before parsing untrusted schema/continuation input');
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
  'tenant', 'actor', 'user_id', 'worker', 'page_limit', 'max_pages', 'heartbeat', 'lease',
  'partition', 'source_name', 'StopHandle', 'is_stopping', 'Uuid',
]) {
  if (runInput.includes(forbidden)) fail(`durable replay input contains caller-owned field marker ${forbidden}`);
}
if (!runInput.includes('locale: Option<String>')) {
  fail('durable replay input must expose only one optional locale scope extension');
}

const shadowInputStart = transport.indexOf('pub struct IndexReplayShadowRunInput');
const shadowInputEnd = transport.indexOf('\n}', shadowInputStart);
const shadowInput = transport.slice(shadowInputStart, shadowInputEnd);
for (const forbidden of [
  'tenant', 'actor', 'worker', 'locale', 'page_limit', 'max_pages', 'heartbeat', 'lease',
  'partition', 'source_name', 'job_id', 'checkpoint', 'cancel', 'retry', 'StopHandle', 'Uuid',
]) {
  if (shadowInput.includes(forbidden)) fail(`Shadow replay input contains caller-owned field marker ${forbidden}`);
}
if (!shadowInput.includes('continuation: Option<String>')) {
  fail('schema-wide Shadow input must expose only one optional sealed continuation extension');
}

const production = transport.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'tokio::spawn',
  '.scan(',
  'PostgresIndexReplayRunner',
  'SharedIndexReplayRuntime',
  'SharedIndexReplayDryRunRuntime',
  'IndexReplayDryRunRequest',
  '.run_shadow(',
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
requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub struct IndexReplayOperatorRuntime',
  'pub async fn run_interruptible<Check>(',
  'pub async fn run_shadow(',
  'pub async fn request_cancel(',
  'IndexReplayShadowTransportRuntime',
  'Permission::MODULES_MANAGE',
]);
requireMarkers('apps/server/src/services/index_replay_shadow_transport.rs', [
  'pub struct IndexReplayShadowTransportRuntime',
  'context.authorize_for(context.tenant_id())?;',
  'IndexSourceContinuationScope::from_registry(',
  'self.operator.run_shadow(context, request).await?',
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
]);
requireMarkers('apps/server/src/services/app_lifecycle.rs', [
  'pub struct StopHandle',
  'pub fn subscribe(&self)',
  'pub async fn stop(&self)',
  'pub fn is_stopping(&self) -> bool',
]);
requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'Status: `full_locale_and_schema_wide_shadow_source_complete_execution_pending`.',
  '`runIndexReplay(input: ...)`',
  '`runIndexReplayShadow(input: ...)`',
  '`cancelIndexReplay(input: ...)`',
  'Tenant and actor identities are never accepted',
  'optional canonicalizable locale',
  'schema-wide Shadow path intentionally has no locale input',
  'page limit: `100` mutations',
  'maximum pages: `8`',
  'lease duration: `60` seconds',
  'same fixed source page limit and maximum-page count (`100 × 8`)',
  '`StopHandle::is_stopping`',
  'maintainer-owned',
]);

console.log('[verify-index-replay-graphql-transport] durable Full/cancel and sealed schema-wide Shadow commands remain authorization-first and server-bounded without transport-owned execution state');
