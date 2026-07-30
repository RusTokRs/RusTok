#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-job-leases] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const jobPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_job.rs';
const job = requireMarkers(jobPath, [
  'pub struct IndexReplayJobLeaseRequest',
  'pub struct IndexReplayJobLease',
  'pub enum IndexReplayJobAcquireOutcome',
  'pub enum IndexReplayJobError',
  'pub struct PostgresIndexReplayJobStore',
  'pub async fn acquire(',
  'pub async fn heartbeat(',
  'pub async fn succeed(',
  'pub async fn fail(',
  'REPLAY_JOB_REQUEST_CONTRACT: &str = "index_replay_job_v1"',
  'pg_advisory_xact_lock(hashtextextended($1, 0))',
  "kind = 'rebuild'",
  "scope_kind = 'schema'",
  "state IN ('pending', 'running', 'succeeded')",
  'attempt_count.checked_add(1)',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'pub(super) async fn assert_active_replay_job_lease(',
  'FOR UPDATE',
  'require_complete_checkpoint(',
  'CheckpointMissing',
  'CheckpointIncomplete',
  "checkpoint_kind = 'rebuild'",
  "locale_key = '' AND partition_key = ''",
]);

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'tokio::spawn',
  'loop {',
  'cancel_requested = TRUE',
  'DELETE FROM index_jobs',
]) {
  if (job.includes(forbidden)) fail(`${jobPath} contains forbidden marker ${forbidden}`);
}

const checkpointPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
const checkpoint = requireMarkers(checkpointPath, [
  'pub struct PostgresIndexReplayCheckpointStore',
  'lease: IndexReplayJobLease',
  'pub fn new(db: DatabaseConnection, lease: IndexReplayJobLease)',
  'validate_checkpoint_identity(&self.lease, key)?;',
  'validate_checkpoint_identity(&self.lease, checkpoint.key())?;',
  'assert_active_replay_job_lease(&transaction, &self.lease, backend)',
  'checkpoint_lease_identity_mismatch',
  'checkpoint_lease_lost',
]);

const checkpointLeasePosition = checkpoint.indexOf(
  'assert_active_replay_job_lease(&transaction, &self.lease, backend)',
);
const checkpointWritePosition = checkpoint.indexOf('upsert_checkpoint_sql(backend)');
if (
  checkpointLeasePosition < 0
  || checkpointWritePosition < 0
  || checkpointLeasePosition > checkpointWritePosition
) {
  fail('checkpoint lease validation must occur before checkpoint persistence');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_job_tests.rs', [
  'replay_job_excludes_other_workers_and_requires_complete_checkpoint',
  'expired_replay_job_is_reclaimed_and_old_checkpoint_writer_is_fenced',
  'failed_terminal_replay_job_allows_a_new_job_identity',
  'replay_job_schema_source_and_stored_request_fail_closed',
  'IndexReplayJobAcquireOutcome::Busy',
  'IndexReplayJobError::CheckpointMissing',
  'IndexReplayJobError::CheckpointIncomplete',
  'checkpoint_lease_lost',
  'second.attempt_count(), 2',
]);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_job;',
  'mod source_replay_job_tests;',
  'PostgresIndexReplayJobStore',
  'IndexReplayJobLeaseRequest',
  'IndexReplayJobAcquireOutcome',
]);

requireMarkers('crates/rustok-index/src/lib.rs', [
  'PostgresIndexReplayJobStore',
  'IndexReplayJobLease',
  'IndexReplayJobLeaseRequest',
  'IndexReplayJobAcquireOutcome',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-job-leases.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`index_replay_job_v1`',
  'attempt count',
  'it cannot advance the durable cursor',
  'JSON `null` cursor',
  'maintainer-run',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- M6 replay job leases and checkpoint attempt fencing: `source_complete_owner_execution_pending`',
  '- [x] Add durable schema-scoped rebuild jobs, lease/heartbeat, reclaim, attempt fencing,',
  '- [x] Bind checkpoint reads and writes to the active `(job_id, worker_id, attempt_count)`',
  '- [ ] Add cancellation, bounded multi-page resume, dry-run, targeted/full/shadow rebuild.',
  'node scripts/verify/verify-index-replay-job-leases.mjs',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-replay-contract.mjs'",
  "'verify-index-replay-job-leases.mjs'",
]);

console.log('[verify-index-replay-job-leases] OK');
