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
  'SELECT job_id, state, request, last_error_code',
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  "WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3",
  'last_error_code is outside the replay error contract',
]);
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
    fail(`${jobPath} dead-letter admission contains ${forbidden}`);
  }
}

const testsPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_job_tests.rs';
requireMarkers(testsPath, [
  'failed_terminal_replay_job_blocks_scope_without_raw_details',
  'Err(IndexReplayJobError::DeadLettered {',
  'error_code: Some("index.replay_source_failed".to_owned()),',
  'SELECT COUNT(*) AS job_count FROM index_jobs',
  'assert_eq!(count, 1);',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-dead-letter-admission.md', [
  'Status: `source_complete_operator_requeue_pending`',
  'A durable\n`failed` rebuild job is treated as a dead letter',
  '`IndexReplayJobError::DeadLettered`',
  '`last_error_details`',
  'ordinary replay\n  acquisition cannot bypass it with a new job UUID',
  'an authorized operator inspect/requeue command',
  'combined implementation-plan item for bounded retry/backoff',
  'maintainer-run',
]);

console.log('[verify-index-replay-dead-letter-admission] OK');
