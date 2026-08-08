#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-targeted-graphql-transport] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'const MAX_TARGETED_KEYS: usize = 256;',
  'pub struct IndexReplayTargetedKeyInput',
  'pub entity_id: String',
  'pub locale: Option<String>',
  'pub struct IndexReplayTargetedRunInput',
  'pub targets: Vec<IndexReplayTargetedKeyInput>',
  'pub struct IndexReplayTargetedRunPayload',
  'pub requested_count: i32',
  'pub mutations_processed: i32',
  'pub missing_count: i32',
  'pub applied_count: i32',
  'pub duplicate_count: i32',
  'pub stale_count: i32',
  'async fn run_index_replay_targeted(',
  'prepare_authorized_targeted_run(tenant.id, auth.user_id, input)',
  '.run_targeted(operator_context, request)',
  '.map_err(map_targeted_operator_error)?',
  'fn prepare_authorized_targeted_run(',
  'let context = authorize(tenant_id, actor_id)?;',
  'let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;',
  'input.targets.is_empty() || input.targets.len() > MAX_TARGETED_KEYS',
  'Uuid::parse_str(bounded_text("entity_id", &target.entity_id, 64)?)',
  'let locale = parse_locale(target.locale)?;',
  'rustok_index::EntityKey {',
  'tenant_id,',
  'schema: schema.clone(),',
  'rustok_index::IndexSourceLoadRequest::new(keys)',
  'fn map_targeted_operator_error(',
  'IndexReplayTargetedOperatorError::Authorization(error) => map_operator_error(error)',
  'IndexReplayTargetedOperatorError::Targeted(error) => map_targeted_error(error)',
  'targeted_transport_authorizes_before_schema_entity_and_locale_parsing',
  'targeted_transport_builds_bounded_canonical_exact_keys_after_authorization',
  'targeted_transport_rejects_empty_oversized_and_duplicate_target_sets_after_authorization',
  'Some("en-US")',
]);

const prepare = graphql.indexOf('fn prepare_authorized_targeted_run(');
const authorize = graphql.indexOf('let context = authorize(tenant_id, actor_id)?;', prepare);
const schema = graphql.indexOf('let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;', authorize);
const bound = graphql.indexOf('input.targets.is_empty() || input.targets.len() > MAX_TARGETED_KEYS', schema);
const entity = graphql.indexOf('Uuid::parse_str(bounded_text("entity_id", &target.entity_id, 64)?)', bound);
const locale = graphql.indexOf('let locale = parse_locale(target.locale)?;', entity);
const request = graphql.indexOf('rustok_index::IndexSourceLoadRequest::new(keys)', locale);
if (prepare < 0 || authorize <= prepare || schema <= authorize || bound <= schema || entity <= bound || locale <= entity || request <= locale) {
  fail('Targeted preparation order must remain authorize -> schema -> target bound -> entity -> locale -> canonical load request');
}

const mutation = graphql.indexOf('async fn run_index_replay_targeted(');
const preparationCall = graphql.indexOf('prepare_authorized_targeted_run(tenant.id, auth.user_id, input)', mutation);
const runtime = graphql.indexOf('let runtime = replay_runtime(ctx)?;', preparationCall);
const guardedRun = graphql.indexOf('.run_targeted(operator_context, request)', runtime);
if (mutation < 0 || preparationCall <= mutation || runtime <= preparationCall || guardedRun <= runtime) {
  fail('Targeted GraphQL must prepare authorized request before resolving and invoking guarded replay operator');
}

const targetedInputStart = graphql.indexOf('pub struct IndexReplayTargetedRunInput');
const targetedInputEnd = graphql.indexOf('\n}', targetedInputStart);
const targetedInput = graphql.slice(targetedInputStart, targetedInputEnd);
for (const forbidden of [
  'tenant', 'actor', 'user_id', 'source_name', 'mode', 'worker', 'page_limit', 'max_pages',
  'heartbeat', 'lease', 'job_id', 'checkpoint', 'cancel', 'retry', 'scheduler', 'partition',
  'continuation', 'StopHandle',
]) {
  if (targetedInput.includes(forbidden)) fail(`Targeted input exposes caller-owned marker ${forbidden}`);
}

const keyStart = graphql.indexOf('pub struct IndexReplayTargetedKeyInput');
const keyEnd = graphql.indexOf('\n}', keyStart);
const keyInput = graphql.slice(keyStart, keyEnd);
for (const forbidden of ['tenant', 'schema', 'source', 'mode', 'worker', 'partition']) {
  if (keyInput.includes(forbidden)) fail(`Targeted key exposes forbidden scope marker ${forbidden}`);
}

const payloadStart = graphql.indexOf('pub struct IndexReplayTargetedRunPayload');
const payloadEnd = graphql.indexOf('\n}', payloadStart);
const payload = graphql.slice(payloadStart, payloadEnd);
for (const forbidden of [
  'source_name', 'job_id', 'checkpoint', 'lease', 'worker', 'retry', 'scheduler', 'partition',
  'event_id', 'source_version',
]) {
  if (payload.includes(forbidden)) fail(`Targeted payload leaks internal marker ${forbidden}`);
}

const production = graphql.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'SharedIndexReplayRuntime',
  'IndexReplayTargetedExecutor',
  'PostgresMutationStore',
  'PostgresIndexReplayRunner',
  'PostgresIndexReplayJobStore',
  'PostgresIndexReplayCheckpointStore',
  'DatabaseConnection',
  'sea_orm',
  'tokio::spawn',
  '.scan(',
  '.load(',
  '.stop()',
]) {
  if (production.includes(forbidden)) fail(`${graphqlPath} bypasses guarded Targeted boundary: ${forbidden}`);
}
const targetedMethodStart = production.indexOf('async fn run_index_replay_targeted(');
const targetedMethodEnd = production.indexOf('    /// Run one server-bounded schema-wide', targetedMethodStart);
const targetedMethod = production.slice(targetedMethodStart, targetedMethodEnd);
for (const forbidden of ['StopHandle', 'is_stopping', 'request_cancel', 'continuation', 'GRAPHQL_REPLAY_PAGE_LIMIT']) {
  if (targetedMethod.includes(forbidden)) fail(`Targeted GraphQL method gained durable/scan lifecycle marker ${forbidden}`);
}

const sourceRegistry = requireMarkers('crates/rustok-index/src/application/source_registry.rs', [
  'const MAX_LOAD_KEYS: usize = 256;',
  'pub struct IndexSourceLoadRequest',
  'IndexSourceError::EmptyLoadKeys',
  'IndexSourceError::TooManyLoadKeys',
  'IndexSourceError::MixedLoadScope',
  'IndexSourceError::DuplicateLoadKey',
]);
if (!sourceRegistry.includes('if keys.len() > MAX_LOAD_KEYS')) {
  fail('canonical load request must enforce MAX_LOAD_KEYS');
}

requireMarkers('apps/server/src/services/index_replay_runtime_composition.rs', [
  'pub enum IndexReplayTargetedOperatorError',
  'Authorization(#[from] IndexReplayOperatorError)',
  'Targeted(#[from] rustok_index::IndexReplayTargetedError)',
  'pub async fn run_targeted(',
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run_targeted(request).await.map_err(Into::into)',
]);
requireMarkers('crates/rustok-index/src/application/targeted_replay.rs', [
  'IndexReplayModeSelection::Targeted(request) => request',
  'source_for_schema(request.schema())',
  '.load(request)',
  'self.schemas.validate_mutation(mutation)',
  '.apply_replay_mutation(self.schemas.as_ref(), &source_name, mutation)',
  'missing_count: requested_count - mutation_count',
]);
requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'Status: `full_shadow_targeted_source_complete_execution_pending`.',
  '`runIndexReplayTargeted(input: ...)`',
  '1..=256',
  'the GraphQL payload does not expose it',
  'Targeted has no durable pending state',
]);
requireMarkers('crates/rustok-index/docs/m6-targeted-replay-mutation-application.md', [
  'Status: `source_complete_transport_execution_pending`.',
  '## GraphQL transport',
  '`runIndexReplayTargeted(input: ...)`',
  'not expose it. Source routing remains server-owned.',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_targeted_graphql_execution_pending`.',
  '## Targeted GraphQL transport',
  '`runIndexReplayTargeted` is a dedicated mutation',
  'No additional independent source-only M6 replay boundary is open',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  '[x] Add a dedicated authorization-first Targeted GraphQL transport over `IndexReplayOperatorRuntime::run_targeted`.',
  'There is no remaining independent source-only M6 replay expansion justified by the current contract.',
]);

console.log('[verify-index-replay-targeted-graphql-transport] Targeted GraphQL is authorization-first, canonical exact-key bounded, source-private and delegated only through the guarded operator without durable replay ownership');
