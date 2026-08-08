#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-shadow-graphql-transport] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const continuationPath = 'crates/rustok-index/src/application/source_continuation.rs';
const continuationSource = requireMarkers(continuationPath, [
  'locale: Option<LocaleKey>',
  'pub fn for_locale(',
  'claims.locale != expected_scope.locale',
  'IndexSourceContinuationError::LocaleScopeMismatch',
  'schema_wide_and_exact_locale_continuations_cannot_cross_scopes',
]);
for (const forbidden of [
  'CONTINUATION_VERSION',
  'ContinuationClaimsV1',
  'ContinuationClaimsV2',
  'UnsupportedVersion',
  'ContractVersionMismatch',
]) {
  if (continuationSource.includes(forbidden)) {
    fail(`${continuationPath} must retain one current unversioned envelope: ${forbidden}`);
  }
}

const servicePath = 'apps/server/src/services/index_replay_shadow_transport.rs';
const service = requireMarkers(servicePath, [
  'pub enum IndexReplayShadowTransportError',
  'pub struct IndexReplayShadowTransportOutcome',
  'pub struct IndexReplayShadowTransportRuntime',
  'pub async fn run_schema_wide(',
  'context.authorize_for(context.tenant_id())?;',
  'IndexSourceContinuationScope::from_registry(',
  '.open_encoded(&scope, encoded, Utc::now())',
  'IndexReplayDryRunRequest::new(',
  'self.operator.run_shadow(context, request).await?',
  '.next_cursor()',
  'codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())',
  'materialize_index_replay_shadow_transport(',
  'extensions.insert(IndexReplayShadowTransportRuntime::new(',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresIndexReplayRunner',
  'PostgresMutationStore',
  'index_jobs',
  'index_checkpoints',
  'request_cancel(',
  'tokio::spawn',
  'tokio::time::sleep',
  '.execute(',
  '.begin()',
  'IndexSourceScanRequest::for_locale',
  'LocaleKey',
]) {
  if (service.includes(forbidden)) {
    fail(`${servicePath} must remain schema-wide, no-write and lifecycle-neutral until the next locale execution slice: ${forbidden}`);
  }
}
const authorize = service.indexOf('context.authorize_for(context.tenant_id())?;');
const scope = service.indexOf('IndexSourceContinuationScope::from_registry(', authorize);
const open = service.indexOf('.open_encoded(&scope, encoded, Utc::now())', scope);
const request = service.indexOf('IndexReplayDryRunRequest::new(', open);
const run = service.indexOf('self.operator.run_shadow(context, request).await?', request);
const seal = service.indexOf('codec.seal(&scope, cursor, Utc::now(), keyring.lifetime())', run);
if (authorize < 0 || scope <= authorize || open <= scope || request <= open || run <= request || seal <= run) {
  fail('Shadow transport order must remain authorize -> frozen schema-wide scope -> open token -> dry-run request -> guarded Shadow -> seal cursor');
}

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'const MAX_CONTINUATION_BYTES: usize = 16 * 1024;',
  'pub struct IndexReplayShadowRunInput',
  'pub continuation: Option<String>',
  'pub enum IndexReplayGraphqlShadowStatus',
  'pub struct IndexReplayShadowRunPayload',
  'async fn run_index_replay_shadow(',
  'prepare_authorized_shadow_run(tenant.id, auth.user_id, input)',
  '.get::<IndexReplayShadowTransportRuntime>()',
  '.run_schema_wide(',
  'GRAPHQL_REPLAY_PAGE_LIMIT,',
  'GRAPHQL_REPLAY_MAX_PAGES,',
  'shadow_transport_authorizes_before_schema_and_continuation_parsing',
  'shadow_transport_accepts_only_schema_and_bounded_sealed_continuation',
  'INDEX_REPLAY_SHADOW_CONTINUATION_INVALID',
  'INDEX_REPLAY_SHADOW_CONTINUATION_EXPIRED',
  'Error::LocaleScopeMismatch',
]);
const shadowPrepare = graphql.indexOf('fn prepare_authorized_shadow_run(');
const shadowAuthorize = graphql.indexOf('let context = authorize(tenant_id, actor_id)?;', shadowPrepare);
const shadowSchema = graphql.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', shadowPrepare);
const shadowContinuation = graphql.indexOf('bounded_text("continuation", &value, MAX_CONTINUATION_BYTES)?;', shadowPrepare);
if (shadowPrepare < 0 || shadowAuthorize < shadowPrepare || shadowSchema <= shadowAuthorize || shadowContinuation <= shadowSchema) {
  fail('Shadow GraphQL preparation must authorize before parsing schema and continuation');
}
const inputStart = graphql.indexOf('pub struct IndexReplayShadowRunInput');
const inputEnd = graphql.indexOf('\n}', inputStart);
const input = graphql.slice(inputStart, inputEnd);
for (const forbidden of [
  'tenant', 'actor', 'worker', 'locale', 'page_limit', 'max_pages', 'heartbeat', 'lease',
  'partition', 'source_name', 'job_id', 'checkpoint', 'cancel', 'retry', 'StopHandle', 'Uuid',
]) {
  if (input.includes(forbidden)) fail(`Shadow GraphQL input contains caller-owned field marker ${forbidden}`);
}
for (const forbidden of [
  'SharedIndexReplayDryRunRuntime',
  'IndexReplayDryRunRequest',
  '.run_shadow(',
  '.scan(',
  'DatabaseConnection',
  'PostgresIndexReplayRunner',
  'PostgresMutationStore',
]) {
  if (graphql.split('\n#[cfg(test)]')[0].includes(forbidden)) {
    fail(`${graphqlPath} must delegate Shadow only through the sealed transport adapter: ${forbidden}`);
  }
}

requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  '#[path = "index_replay_shadow_transport.rs"]',
  'IndexReplayShadowTransportRuntime',
  'materialize_index_replay_shadow_transport(',
  'continuation.clone()',
]);
requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'Status: `full_locale_schema_wide_shadow_and_locale_safe_continuation_source_complete_execution_pending`.',
  '`runIndexReplayShadow(input: ...)`',
  'current Shadow GraphQL path intentionally still has no locale input',
  'same fixed source page limit and maximum-page count (`100 × 8`)',
  'continuation contract itself is now locale-safe',
  'one current unversioned envelope',
]);
requireMarkers('crates/rustok-index/docs/m6-bounded-replay-dry-run.md', [
  'Status: `source_complete_schema_wide_transport_locale_execution_pending`',
  '`runIndexReplayShadow`',
  'continuation scope now binds optional canonical locale identity',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_shadow_schema_wide_transport_locale_execution_pending`.',
  '`runIndexReplayShadow`',
  'Locale-safe continuation identity',
  'Exact-locale Shadow transport remains source-open',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.',
  'Add exact-locale Shadow dry-run/runtime/GraphQL execution using the canonical locale-safe continuation scope.',
]);

console.log('[verify-index-replay-shadow-graphql-transport] schema-wide Shadow GraphQL remains authorization-first/no-write while the single canonical continuation format now binds exact locale scope');
