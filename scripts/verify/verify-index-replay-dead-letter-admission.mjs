#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-dead-letter-admission] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const jobPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_job.rs';
const jobs = requireMarkers(jobPath, [
  'DeadLettered {',
  'job_id: Uuid,',
  'attempt_count: u32,',
  'error_code: Option<String>,',
  '"failed" => {',
  'error_code: stored.last_error_code,',
  'last_error_code: Option<String>,',
  'last_error_code is outside the replay error contract',
]);

const selectStart = jobs.indexOf('fn select_replay_jobs_sql(');
const selectEnd = jobs.indexOf('\nfn insert_job_sql(', selectStart);
if (selectStart < 0 || selectEnd <= selectStart) {
  fail(`${jobPath} must retain one bounded replay-job selection function`);
}
const select = jobs.slice(selectStart, selectEnd);
for (const marker of [
  'SELECT job_id, state,',
  'request, last_error_code',
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  "WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3",
  'created_at DESC',
]) {
  if (!select.includes(marker)) fail(`${jobPath} replay-job selection is missing ${marker}`);
}
if (select.includes('last_error_details')) {
  fail(`${jobPath} dead-letter admission must not select last_error_details`);
}

const succeededOrder = select.indexOf("WHEN 'succeeded' THEN 0");
const runningOrder = select.indexOf("WHEN 'running' THEN 1");
const pendingOrder = select.indexOf("WHEN 'pending' THEN 2");
const failedOrder = select.indexOf('ELSE 3');
if (
  succeededOrder < 0
  || runningOrder <= succeededOrder
  || pendingOrder <= runningOrder
  || failedOrder <= pendingOrder
) {
  fail(`${jobPath} must preserve succeeded/running/pending/failed admission precedence`);
}

const deadLetterStart = jobs.indexOf('"failed" => {');
const deadLetterEnd = jobs.indexOf('state => {', deadLetterStart);
if (deadLetterStart < 0 || deadLetterEnd <= deadLetterStart) {
  fail(`${jobPath} dead-letter match block is missing`);
}
const deadLetterBlock = jobs.slice(deadLetterStart, deadLetterEnd);
for (const forbidden of [
  'last_error_details',
  'error_details',
  'INSERT INTO index_jobs',
  'Storage(',
]) {
  if (deadLetterBlock.includes(forbidden)) {
    fail(`${jobPath} dead-letter admission contains forbidden marker ${forbidden}`);
  }
}
if (jobs.includes("UPDATE index_jobs SET state = 'pending'")) {
  fail(`${jobPath} must not implement an unauthorized failed-to-pending requeue transition`);
}

const testsPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_job_tests.rs';
requireMarkers(testsPath, [
  'failed_terminal_replay_job_blocks_scope_without_raw_details',
  'Err(IndexReplayJobError::DeadLettered {',
  'error_code: Some("index.replay_source_failed".to_owned()),',
  '"private": "must-not-be-returned"',
  'SELECT COUNT(*) AS job_count FROM index_jobs',
  'assert_eq!(count, 1);',
  'replay_job_excludes_other_workers_and_requires_complete_checkpoint',
  'expired_replay_job_is_reclaimed_and_old_checkpoint_writer_is_fenced',
]);

requireMarkers(
  'crates/rustok-index/src/infrastructure/postgres/source_replay_retry.rs',
  [
    'pub struct PostgresIndexReplayRetryStore',
    'IndexReplayRetryDisposition::TerminalPermanent',
    'IndexReplayRetryDisposition::TerminalExhausted',
    "state = 'failed'",
  ],
);

const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'match finish_failure(&self.db, &lease, details).await?',
]);
for (const premature of [
  'PostgresIndexReplayRetryStore',
  'IndexReplayRetryFailure',
  '.record_failure(',
]) {
  if (runner.includes(premature)) {
    fail(`${runnerPath} prematurely claims retry-store wiring through ${premature}`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-replay-dead-letter-admission.md', [
  'Status: `source_complete_operator_requeue_pending`',
  '`failed` rebuild job becomes the ordinary replay admission barrier',
  '`IndexReplayJobError::DeadLettered`',
  'The SELECT does not load `last_error_details`',
  'ordinary acquisition cannot bypass it with a new job UUID',
  'authorized requeue or a failed-to-pending transition',
  'canonical implementation-plan item',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Replay Dead-letter Admission](./m6-replay-dead-letter-admission.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-dead-letter-admission.mjs'",
]);

console.log('[verify-index-replay-dead-letter-admission] OK');
