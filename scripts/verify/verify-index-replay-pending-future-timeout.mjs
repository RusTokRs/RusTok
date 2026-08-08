#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-pending-future-timeout] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const helperPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_timeout.rs';
const helper = requireMarkers(helperPath, [
  'DEFAULT_INDEX_REPLAY_STORAGE_FUTURE_TIMEOUT: Duration = Duration::from_secs(30)',
  'INDEX_REPLAY_MUTATION_TIMEOUT_CODE: &str = "index_replay_mutation_timeout"',
  'INDEX_REPLAY_CHECKPOINT_READ_TIMEOUT_CODE: &str = "index_replay_checkpoint_read_timeout"',
  '"index_replay_checkpoint_commit_timeout"',
  'tokio::time::timeout',
  'bounded_replay_mutation',
  'bounded_replay_checkpoint_read',
  'bounded_replay_checkpoint_commit',
  'IndexReplayFailure::retryable_static(timeout_code)',
  'pending::<Result<(), IndexReplayFailure>>()',
  'pending_checkpoint_read_future_times_out_as_retryable',
  'IndexReplayFailureKind::Retryable',
  'dependency_failure_is_preserved_before_timeout',
]);
for (const forbidden of ['StopHandle', 'request_cancel', 'cancel_requested', 'yield_for_resume']) {
  if (helper.includes(forbidden)) {
    fail(`${helperPath} must stay storage-timeout-only and not absorb lifecycle/cancellation semantics: ${forbidden}`);
  }
}

const adapterPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
const adapter = requireMarkers(adapterPath, [
  'bounded_replay_checkpoint_commit, bounded_replay_checkpoint_read, bounded_replay_mutation',
  'bounded_replay_mutation(async {',
  'self.apply(registry, &delivery)',
  'bounded_replay_checkpoint_read(async {',
  'bounded_replay_checkpoint_commit(async {',
  'validate_checkpoint_identity(&self.lease, key)?;',
  'validate_checkpoint_identity(&self.lease, checkpoint.key())?;',
  'assert_active_replay_job_lease(&transaction, &self.lease, backend)',
  'upsert_checkpoint_sql(backend)',
]);
const readIdentity = adapter.indexOf('validate_checkpoint_identity(&self.lease, key)?;');
const readTimeout = adapter.indexOf('bounded_replay_checkpoint_read(async {', readIdentity);
if (readIdentity < 0 || readTimeout <= readIdentity) {
  fail('checkpoint read identity validation must remain outside/before the bounded read future');
}
const commitIdentity = adapter.indexOf('validate_checkpoint_identity(&self.lease, checkpoint.key())?;');
const commitTimeout = adapter.indexOf('bounded_replay_checkpoint_commit(async {', commitIdentity);
if (commitIdentity < 0 || commitTimeout <= commitIdentity) {
  fail('checkpoint commit identity validation must remain outside/before the bounded commit future');
}

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'let (page_result, in_page_heartbeat_count) = await_page_with_lease_heartbeats(',
  'let page = match page_result {',
  'if cancel_if_requested(&self.db, &lease).await? {',
  'let details = replay_failure_details(&error);',
  'finish_failure(&self.db, &lease, details).await?',
  'IndexReplayError::MutationFailed { failure, .. }',
  '| IndexReplayError::CheckpointReadFailed(failure)',
  '| IndexReplayError::CheckpointCommitFailed(failure)',
  'failure.kind() == IndexReplayFailureKind::Retryable',
  '"retryable": retryable',
]);
const pageResult = runner.indexOf('let page = match page_result {');
const pageFailure = runner.indexOf('Err(error) => {', pageResult);
const cancelCheck = runner.indexOf('if cancel_if_requested(&self.db, &lease).await? {', pageFailure);
const failureDetails = runner.indexOf('let details = replay_failure_details(&error);', cancelCheck);
const finishFailure = runner.indexOf('finish_failure(&self.db, &lease, details).await?', failureDetails);
if (
  pageResult < 0 ||
  pageFailure <= pageResult ||
  cancelCheck <= pageFailure ||
  failureDetails <= cancelCheck ||
  finishFailure <= failureDetails
) {
  fail('persisted cancellation must keep precedence over terminal page failure after a timeout');
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_timeout;',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-pending-future-timeouts.md', [
  'Status: `source_complete_execution_pending`.',
  '`index_replay_mutation_timeout`',
  '`index_replay_checkpoint_read_timeout`',
  '`index_replay_checkpoint_commit_timeout`',
  'does **not** prove that the database operation was rolled back or cancelled',
  'user cancellation that won the race remains `Cancelled`',
  '`retryable: true`',
  '`StopHandle` interruption remains a separate safe-point-only',
  '`m6-replay-page-lease-heartbeat.md`',
]);

console.log('[verify-index-replay-pending-future-timeout] replay checkpoint-read/mutation/checkpoint-commit futures retain bounded retryable identities while lease, cancel, and graceful-stop semantics stay separate');
