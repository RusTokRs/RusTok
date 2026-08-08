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
  'pub struct IndexReplayTargetedKeyInput',
  'pub entity_id: String',
  'pub struct IndexReplayTargetedRunInput',
  'pub targets: Vec<IndexReplayTargetedKeyInput>',
  'pub struct IndexReplayShadowRunInput',
  'pub continuation: Option<String>',
  'pub struct IndexReplayCancelInput',
  'pub job_id: String',
  'const MAX_LOCALE_BYTES: usize = 32;',
  'const MAX_CONTINUATION_BYTES: usize = 16 * 1024;',
  'const MAX_TARGETED_KEYS: usize = 256;',
  'const GRAPHQL_REPLAY_PAGE_LIMIT: usize = 100;',
  'const GRAPHQL_REPLAY_MAX_PAGES: usize = 8;',
  'const GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES: usize = 1;',
  'const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;',
  'async fn run_index_replay(',
  'async fn run_index_replay_targeted(',
  'async fn run_index_replay_shadow(',
  'async fn cancel_index_replay(',
  'prepare_authorized_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_targeted_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_shadow_run(tenant.id, auth.user_id, input)',
  'prepare_authorized_cancel(tenant.id, auth.user_id, input)',
  'permissions_for(&tenant_id, &actor_id)',
  'has_effective_permission(&permissions, &Permission::MODULES_MANAGE)',
  'rustok_index::LocaleKey::new(locale)',
  'rustok_index::IndexReplayRunRequest::for_locale(',
  'rustok_index::IndexReplayRunRequest::new(',
  'rustok_index::IndexSourceLoadRequest::new(keys)',
  'let worker_id = format!("graphql-replay-{}", Uuid::new_v4().simple());',
  '.get::<IndexReplayOperatorRuntime>()',
  '.get::<IndexReplayShadowTransportRuntime>()',
  'let stop_handle = ctx.data::<StopHandle>()?.clone();',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  '.run_targeted(operator_context, request)',
  '.run(',
  '.request_cancel(operator_context, job_id)',
  'replay_transport_authorizes_before_parsing_untrusted_run_input',
  'targeted_transport_authorizes_before_schema_entity_and_locale_parsing',
  'targeted_transport_builds_bounded_canonical_exact_keys_after_authorization',
  'targeted_transport_rejects_empty_oversized_and_duplicate_target_sets_after_authorization',
  'shadow_transport_authorizes_before_schema_locale_and_continuation_parsing',
  'shadow_transport_accepts_schema_locale_and_bounded_sealed_continuation',
  'replay_transport_derives_authority_worker_and_server_owned_budgets',
  'replay_transport_canonicalizes_optional_locale_after_authorization',
  'replay_cancel_authorizes_before_job_id_parsing_and_derives_tenant',
  'Error::LocaleScopeMismatch',
  'IndexReplayDryRunError::LocaleScopeUnsupported',
]);

const runPrepare = transport.indexOf('fn prepare_authorized_run(');
const runAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', runPrepare);
const runSchemaParse = transport.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', runPrepare);
const runLocaleParse = transport.indexOf('let locale = parse_locale(input.locale)?;', runPrepare);
if (runPrepare < 0 || runAuthorize < runPrepare || runSchemaParse <= runAuthorize || runLocaleParse <= runAuthorize) {
  fail('durable Full transport must authorize before parsing untrusted schema/locale input');
}

const targetedPrepare = transport.indexOf('fn prepare_authorized_targeted_run(');
const targetedAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', targetedPrepare);
const targetedSchemaParse = transport.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', targetedPrepare);
const targetedBound = transport.indexOf('input.targets.len() > MAX_TARGETED_KEYS', targetedSchemaParse);
const targetedEntityParse = transport.indexOf('Uuid::parse_str(bounded_text("entity_id"', targetedBound);
const targetedLocaleParse = transport.indexOf('let locale = parse_locale(target.locale)?;', targetedEntityParse);
const targetedRequest = transport.indexOf('rustok_index::IndexSourceLoadRequest::new(keys)', targetedLocaleParse);
if (
  targetedPrepare < 0 ||
  targetedAuthorize < targetedPrepare ||
  targetedSchemaParse <= targetedAuthorize ||
  targetedBound <= targetedSchemaParse ||
  targetedEntityParse <= targetedBound ||
  targetedLocaleParse <= targetedEntityParse ||
  targetedRequest <= targetedLocaleParse
) {
  fail('Targeted transport must authorize -> parse schema -> bound targets -> parse entity/locale -> build canonical load request');
}

const shadowPrepare = transport.indexOf('fn prepare_authorized_shadow_run(');
const shadowAuthorize = transport.indexOf('let context = authorize(tenant_id, actor_id)?;', shadowPrepare);
const shadowSchemaParse = transport.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', shadowPrepare);
const shadowLocaleParse = transport.indexOf('let locale = parse_locale(input.locale)?;', shadowSchemaParse);
const shadowContinuationParse = transport.indexOf('bounded_text("continuation", &value, MAX_CONTINUATION_BYTES)?;', shadowLocaleParse);
if (
  shadowPrepare < 0 ||
  shadowAuthorize < shadowPrepare ||
  shadowSchemaParse <= shadowAuthorize ||
  shadowLocaleParse <= shadowSchemaParse ||
  shadowContinuationParse <= shadowLocaleParse
) {
  fail('Shadow transport must authorize before parsing untrusted schema/locale/continuation input');
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
  'partition', 'source_name', 'StopHandle', 'is_stopping', 'Uuid', 'mode',
]) {
  if (runInput.includes(forbidden)) fail(`durable replay input contains caller-owned field marker ${forbidden}`);
}
if (!runInput.includes('locale: Option<String>')) {
  fail('durable replay input must expose one optional locale scope extension');
}

const targetedInputStart = transport.indexOf('pub struct IndexReplayTargetedRunInput');
const targetedInputEnd = transport.indexOf('\n}', targetedInputStart);
const targetedInput = transport.slice(targetedInputStart, targetedInputEnd);
for (const forbidden of [
  'tenant', 'actor', 'user_id', 'worker', 'page_limit', 'max_pages', 'heartbeat', 'lease',
  'partition', 'source_name', 'job_id', 'checkpoint', 'cancel', 'retry', 'StopHandle',
  'continuation', 'mode',
]) {
  if (targetedInput.includes(forbidden)) fail(`Targeted replay input contains caller-owned field marker ${forbidden}`);
}
for (const required of ['module_name: String', 'entity_name: String', 'schema_version: String', 'targets: Vec<IndexReplayTargetedKeyInput>']) {
  if (!targetedInput.includes(required)) fail(`Targeted replay input is missing ${required}`);
}

const targetedKeyStart = transport.indexOf('pub struct IndexReplayTargetedKeyInput');
const targetedKeyEnd = transport.indexOf('\n}', targetedKeyStart);
const targetedKey = transport.slice(targetedKeyStart, targetedKeyEnd);
for (const required of ['entity_id: String', 'locale: Option<String>']) {
  if (!targetedKey.includes(required)) fail(`Targeted key input is missing ${required}`);
}
for (const forbidden of ['tenant', 'schema', 'source', 'mode', 'worker', 'partition']) {
  if (targetedKey.includes(forbidden)) fail(`Targeted key input contains forbidden caller scope marker ${forbidden}`);
}

const targetedPayloadStart = transport.indexOf('pub struct IndexReplayTargetedRunPayload');
const targetedPayloadEnd = transport.indexOf('\n}', targetedPayloadStart);
const targetedPayload = transport.slice(targetedPayloadStart, targetedPayloadEnd);
for (const required of [
  'requested_count: i32', 'mutations_processed: i32', 'missing_count: i32',
  'applied_count: i32', 'duplicate_count: i32', 'stale_count: i32',
]) {
  if (!targetedPayload.includes(required)) fail(`Targeted payload is missing ${required}`);
}
for (const forbidden of ['source_name', 'job_id', 'checkpoint', 'lease', 'worker', 'retry', 'partition']) {
  if (targetedPayload.includes(forbidden)) fail(`Targeted payload exposes internal marker ${forbidden}`);
}

const shadowInputStart = transport.indexOf('pub struct IndexReplayShadowRunInput');
const shadowInputEnd = transport.indexOf('\n}', shadowInputStart);
const shadowInput = transport.slice(shadowInputStart, shadowInputEnd);
for (const forbidden of [
  'tenant', 'actor', 'worker', 'page_limit', 'max_pages', 'heartbeat', 'lease',
  'partition', 'source_name', 'job_id', 'checkpoint', 'cancel', 'retry', 'StopHandle', 'Uuid', 'mode',
]) {
  if (shadowInput.includes(forbidden)) fail(`Shadow replay input contains caller-owned field marker ${forbidden}`);
}
for (const required of ['locale: Option<String>', 'continuation: Option<String>']) {
  if (!shadowInput.includes(required)) fail(`Shadow replay input is missing ${required}`);
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
  'IndexReplayTargetedExecutor',
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
  'pub enum IndexReplayTargetedOperatorError',
  'pub async fn run_interruptible<Check>(',
  'pub async fn run_targeted(',
  'pub async fn run_shadow(',
  'pub async fn request_cancel(',
  'IndexReplayShadowTransportRuntime',
  'Permission::MODULES_MANAGE',
]);
requireMarkers('apps/server/src/services/index_replay_shadow_transport.rs', [
  'pub struct IndexReplayShadowTransportRuntime',
  'locale: Option<rustok_index::LocaleKey>',
  'context.authorize_for(context.tenant_id())?;',
  'IndexSourceContinuationScope::for_locale(',
  'IndexSourceContinuationScope::from_registry(',
  'IndexReplayDryRunRequest::for_locale(',
  'self.operator.run_shadow(context, request).await?',
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
]);
requireMarkers('crates/rustok-index/src/application/source_registry.rs', [
  'const MAX_LOAD_KEYS: usize = 256;',
  'pub struct IndexSourceLoadRequest',
  'IndexSourceError::EmptyLoadKeys',
  'IndexSourceError::TooManyLoadKeys',
  'IndexSourceError::DuplicateLoadKey',
]);
requireMarkers('apps/server/src/services/app_lifecycle.rs', [
  'pub struct StopHandle',
  'pub fn subscribe(&self)',
  'pub async fn stop(&self)',
  'pub fn is_stopping(&self) -> bool',
]);
requireMarkers('crates/rustok-index/src/application/source_continuation.rs', [
  'pub fn for_locale(',
  'claims.locale != expected_scope.locale',
  'IndexSourceContinuationError::LocaleScopeMismatch',
]);
requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'Status: `full_shadow_targeted_source_complete_execution_pending`.',
  '`runIndexReplay(input: ...)`',
  '`runIndexReplayTargeted(input: ...)`',
  '`runIndexReplayShadow(input: ...)`',
  '`cancelIndexReplay(input: ...)`',
  'Tenant and actor identities are never accepted',
  '1..=256',
  'does not expose it',
  'page limit: `100` mutations',
  'maximum pages: `8`',
  'lease duration: `60` seconds',
  'same fixed source page limit and maximum-page count (`100 × 8`)',
  '`IndexSourceContinuationScope::for_locale`',
  '`StopHandle::is_stopping`',
  'maintainer-owned',
]);

console.log('[verify-index-replay-graphql-transport] Full/cancel, exact-key Targeted, and sealed locale-aware Shadow commands remain authorization-first, dedicated and bounded without transport-owned execution state');
