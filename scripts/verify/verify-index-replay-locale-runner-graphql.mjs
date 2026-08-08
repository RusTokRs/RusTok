#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-locale-runner-graphql] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'pub fn for_locale(',
  'pub fn locale(&self) -> Option<&LocaleKey>',
  'fn lease_request_for_run(',
  'IndexReplayJobLeaseRequest::for_locale(',
  'IndexReplayJobLeaseRequest::new(',
  'lease.locale()',
  'checkpoint.locale_key = {prefix}9',
  "checkpoint.partition_key = ''",
  'await_page_with_lease_heartbeats(',
]);
if (runner.includes("checkpoint.locale_key = ''")) {
  fail(`${runnerPath} must not hard-code schema locale in the terminal success fence`);
}
for (const forbidden of ['partition_key()', 'partition: Option', 'targeted_rebuild', 'shadow_rebuild']) {
  if (runner.includes(forbidden)) fail(`${runnerPath} absorbed forbidden later scope: ${forbidden}`);
}

const gracefulPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_runner/graceful_shutdown.rs';
const graceful = requireMarkers(gracefulPath, [
  'let lease_request = lease_request_for_run(&request, source_name)?;',
  'let page_future = worker.run_next_page_interruptible(',
  'request.page_request().clone(),',
  'await_page_with_lease_heartbeats(',
  'yield_after_host_interruption',
]);
if (graceful.includes('IndexReplayJobLeaseRequest::new(')) {
  fail(`${gracefulPath} must use the common scoped lease helper`);
}

const graphqlPath = 'apps/server/src/graphql/index_replay.rs';
const graphql = requireMarkers(graphqlPath, [
  'const MAX_LOCALE_BYTES: usize = 32;',
  'pub locale: Option<String>',
  'let context = authorize(tenant_id, actor_id)?;',
  'let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;',
  'let locale = parse_locale(input.locale)?;',
  'rustok_index::LocaleKey::new(locale)',
  'rustok_index::IndexReplayRunRequest::for_locale(',
  'rustok_index::IndexReplayRunRequest::new(',
  'locale: Some("EN-us".to_owned())',
  'Some("en-US")',
]);
const authorization = graphql.indexOf('let context = authorize(tenant_id, actor_id)?;');
const schemaParse = graphql.indexOf('let schema = parse_schema(', authorization);
const localeParse = graphql.indexOf('let locale = parse_locale(input.locale)?;', authorization);
if (authorization < 0 || schemaParse <= authorization || localeParse <= authorization) {
  fail(`${graphqlPath} must authorize before parsing schema or locale input`);
}
for (const forbidden of ['pub partition', 'partition_key', 'targeted_rebuild', 'shadow_rebuild']) {
  if (graphql.includes(forbidden)) fail(`${graphqlPath} exposed forbidden later scope: ${forbidden}`);
}

requireMarkers('apps/server/docs/index-replay-graphql-transport.md', [
  'optional canonicalizable locale',
  'Omission is not inferred from schema metadata',
  'The terminal success fence checks the checkpoint using the leased locale',
  '`partition_key`',
]);
requireMarkers('crates/rustok-index/docs/m6-locale-replay-runner-graphql.md', [
  'runner_graphql_source_complete_execution_pending',
  'checkpoint.locale_key',
  'Partition replay must remain blocked',
  'retained end-to-end locale replay/restart execution',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Carry optional locale through the multi-page replay runner and GraphQL command transport',
  'Add partition replay scope only after a real partition-capable source contract exists',
  'Add explicit targeted/full/shadow rebuild modes under a separate contract',
]);

console.log('[verify-index-replay-locale-runner-graphql] locale replay identity remains stable while ordinary and graceful pages share the lease-maintenance helper');
