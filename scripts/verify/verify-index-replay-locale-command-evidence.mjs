#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-locale-command-evidence] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const packetPath = 'apps/server/src/graphql/index_replay_locale_tests.rs';
const packet = requireMarkers(packetPath, [
  'use super::index_replay::IndexReplayMutation;',
  'materialize_index_replay_runtime',
  'PostgresSchemaRegistrationStore',
  'const EN_PAGE_COUNT: usize = 9;',
  'const DE_PAGE_COUNT: usize = 2;',
  'locale_mode: LocaleMode::Required',
  'Some("en-US") => locale_page(',
  'Some("de") => locale_page(',
  'None => {',
  'IndexSourcePage::new(&request, mutations, next_cursor)',
  'graphql_locale_replay_yields_isolates_scopes_and_fresh_runtime_resumes_same_job',
  'execute_replay(&first_runtime.schema, Some("EN-us"))',
  'assert_eq!(first_en["status"], "YIELDED");',
  'assert_eq!(checkpoint_cursor_text(&db, "en-US").await, "8");',
  'execute_replay(&first_runtime.schema, Some("de"))',
  'assert_ne!(de_job_id, en_job_id);',
  'assert_eq!(job_state(&db, en_job_id).await, "pending");',
  'let schema_wide = execute_replay(&first_runtime.schema, None).await;',
  'assert_ne!(schema_job_id, en_job_id);',
  'assert_ne!(schema_job_id, de_job_id);',
  'assert_eq!(schema_wide["appliedCount"], 1);',
  '(EN_PAGE_COUNT + DE_PAGE_COUNT - 1) as i64',
  'assert_eq!(checkpoint_json_type(&db, "").await, "null");',
  'let restarted_runtime = graphql_runtime(&db).await;',
  'let resumed_en = execute_replay(&restarted_runtime.schema, Some("en-US")).await;',
  'assert_eq!(resumed_en["jobId"], en_job_id.to_string());',
  'assert_eq!(resumed_en["duplicateCount"], 1);',
  'assert_eq!(job_attempt_count(&db, en_job_id).await, 2);',
  'assert_eq!(checkpoint_count(&db).await, 3);',
  'assert_eq!(succeeded_replay_job_count(&db).await, 3);',
]);

for (const forbidden of [
  'tokio::time::sleep',
  'std::thread::sleep',
  'interval(',
  'sleep_until(',
  'while !',
  'loop {',
  'PostgresIndexReplayRunner',
  'PostgresIndexReplayJobStore',
  'IndexReplayJobLeaseRequest',
  'INSERT INTO index_jobs',
  'INSERT INTO index_checkpoints',
  '.run_interruptible(',
  'request_cancel(',
  '.stop()',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must retain GraphQL-owned deterministic restart evidence without direct runner/job/checkpoint manipulation: ${forbidden}`);
  }
}

const firstEn = packet.indexOf('execute_replay(&first_runtime.schema, Some("EN-us"))');
const firstYield = packet.indexOf('assert_eq!(first_en["status"], "YIELDED");', firstEn);
const deRun = packet.indexOf('execute_replay(&first_runtime.schema, Some("de"))', firstYield);
const schemaRun = packet.indexOf('let schema_wide = execute_replay(&first_runtime.schema, None).await;', deRun);
const restart = packet.indexOf('let restarted_runtime = graphql_runtime(&db).await;', schemaRun);
const resume = packet.indexOf('execute_replay(&restarted_runtime.schema, Some("en-US"))', restart);
const attemptTwo = packet.indexOf('job_attempt_count(&db, en_job_id).await, 2', resume);
if (
  firstEn < 0 ||
  firstYield <= firstEn ||
  deRun <= firstYield ||
  schemaRun <= deRun ||
  restart <= schemaRun ||
  resume <= restart ||
  attemptTwo <= resume
) {
  fail('evidence order must remain en-US yield -> de completion -> schema completion -> fresh-runtime en-US attempt 2');
}

requireMarkers('apps/server/src/graphql/mod.rs', [
  '#[cfg(test)]\nmod index_replay_locale_tests;'.replace('\\n', '\n'),
]);
requireMarkers('crates/rustok-index/docs/m6-locale-replay-command-evidence.md', [
  'Status: `source_complete_execution_pending`.',
  '`en-US`: 9 one-mutation pages',
  '`de`: 2 one-mutation pages',
  'schema-wide scope exposes the same 11 stable mutations',
  'attempt 2',
  '`Duplicate`',
  'exactly three replay jobs and three checkpoints',
  'Execution and admission remain maintainer-owned',
]);
requireMarkers('crates/rustok-index/docs/m6-locale-replay-runner-graphql.md', [
  'runner_graphql_evidence_source_complete_execution_pending',
  'end-to-end GraphQL locale yield/isolation/fresh-runtime resume',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Retain deterministic locale replay/restart command evidence through the real GraphQL/runtime/runner path.',
  'Execute/admit retained locale replay/restart command evidence, including schema/locale isolation.',
  'Define/retain whole-page duration versus lease/heartbeat policy beyond per-dependency bounds.',
]);

console.log('[verify-index-replay-locale-command-evidence] retained GraphQL locale yield/isolation/restart evidence remains deterministic, durable-scope exact and runner-bypass free');
