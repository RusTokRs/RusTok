#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-shadow-host-dispatch] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const serverPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const server = requireMarkers(serverPath, [
  'pub enum IndexReplayShadowOperatorError',
  'Authorization(#[from] IndexReplayOperatorError)',
  'DryRun(#[from] rustok_index::IndexReplayDryRunError)',
  'shadow: rustok_index::SharedIndexReplayDryRunRuntime',
  'pub async fn run_shadow(',
  'request: rustok_index::IndexReplayDryRunRequest',
  'Result<rustok_index::IndexReplayDryRunOutcome, IndexReplayShadowOperatorError>',
  'context.authorize_for(request.tenant_id())?;',
  'self.shadow.run(request).await.map_err(Into::into)',
  '.get::<rustok_index::SharedIndexReplayDryRunRuntime>()',
  'IndexReplayOperatorRuntime::new(runtime, shadow)',
  'shadow_dispatch_reuses_request_bound_modules_manage_guard',
  'vec![Permission::MODULES_READ]',
  'IndexReplayShadowOperatorError::Authorization(IndexReplayOperatorError::Forbidden)',
  'vec![Permission::MODULES_MANAGE]',
  'IndexReplayDryRunStatus::Complete',
]);
for (const forbidden of ['tokio::spawn', 'tokio::time::sleep', 'loop {']) {
  if (server.includes(forbidden)) fail(`${serverPath} must not create a Shadow scheduler/lifecycle owner: ${forbidden}`);
}

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'async fn run_index_replay(',
  '.run_interruptible(operator_context, request, || stop_handle.is_stopping())',
  'async fn run_index_replay_shadow(',
  '.get::<IndexReplayShadowTransportRuntime>()',
  '.run_schema_wide(',
  'async fn cancel_index_replay(',
]);
for (const forbidden of [
  'SharedIndexReplayDryRunRuntime',
  'IndexReplayDryRunRequest',
  '.run_shadow(',
]) {
  if (graphql.split('\n#[cfg(test)]')[0].includes(forbidden)) {
    fail(`${graphqlPath} must not bypass the sealed Shadow transport adapter: ${forbidden}`);
  }
}

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = read(runnerPath);
for (const forbidden of ['IndexReplayMode::Shadow', 'SideEffectFreeScan', 'SharedIndexReplayDryRunRuntime']) {
  if (runner.includes(forbidden)) {
    fail(`${runnerPath} must remain the durable Full replay runner: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-bounded-replay-dry-run.md', [
  'Status: `source_complete_schema_wide_transport_locale_execution_pending`',
  '`IndexReplayOperatorRuntime::run_shadow`',
  'same request-bound `modules:manage` authorization boundary',
  '`runIndexReplayShadow`',
  'Exact-locale Shadow remains source-open only because the dry-run request/runtime and GraphQL adapter',
]);
requireMarkers('crates/rustok-index/docs/m6-replay-mode-contract.md', [
  'Status: `source_complete_shadow_schema_wide_transport_locale_execution_pending`.',
  '`Shadow` host dispatch is source-complete',
  '`IndexReplayOperatorRuntime::run_shadow`',
  '`runIndexReplayShadow` is a dedicated transport',
  'Locale-safe continuation identity',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.',
  'Add authorization-first schema-wide GraphQL transport for guarded Shadow replay with sealed caller-carried continuation.',
  'Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.',
  'Add exact-locale Shadow dry-run/runtime/GraphQL execution using the canonical locale-safe continuation scope.',
  'Targeted execution remains separate until a bounded mutation-application contract over `IndexSource::load` exists.',
]);

console.log('[verify-index-replay-shadow-host-dispatch] Shadow host dispatch remains modules:manage-guarded/no-write; continuation is locale-safe while execution remains schema-wide until the next slice');
