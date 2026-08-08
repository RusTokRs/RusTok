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
  '"index_replay_checkpoint_commit_timeout"',
  'tokio::time::timeout',
  'bounded_replay_mutation',
  'bounded_replay_checkpoint_commit',
  'IndexReplayFailure::retryable_static(timeout_code)',
  'pending::<Result<(), IndexReplayFailure>>()',
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
  'source_replay_timeout::{bounded_replay_checkpoint_commit, bounded_replay_mutation}',
  'bounded_replay_mutation(async {',
  'self.apply(registry, &delivery)',
  'bounded_replay_checkpoint_commit(async {',
  'validate_checkpoint_identity(&self.lease, checkpoint.key())?;',
  'assert_active_replay_job_lease(&transaction, &self.lease, backend)',
  'upsert_checkpoint_sql(backend)',
  'transaction\n                    .commit()'.replace('\\n', '\n'),
]);
const identityValidation = adapter.indexOf('validate_checkpoint_identity(&self.lease, checkpoint.key())?;');
const checkpointTimeout = adapter.indexOf('bounded_replay_checkpoint_commit(async {', identityValidation);
if (identityValidation < 0 || checkpointTimeout <= identityValidation) {
  fail('checkpoint identity validation must remain outside/before the bounded commit future');
}
if (adapter.includes('bounded_replay_checkpoint_commit(async {\n            validate_checkpoint_identity'.replace('\\n', '\n'))) {
  fail('checkpoint identity validation must not be delayed inside the timeout future');
}

const runnerPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'let page = match worker.run_next_page(request.page_request().clone()).await {',
  'if cancel_if_requested(&self.db, &lease).await? {',
  'let details = replay_failure_details(&error);',
  'finish_failure(&self.db, &lease, details).await?',
  'IndexReplayError::MutationFailed { failure, .. }',
  '| IndexReplayError::CheckpointCommitFailed(failure)',
  'failure.kind() == IndexReplayFailureKind::Retryable',
  '"retryable": retryable',
]);
const pageMatch = runner.indexOf('let page = match worker.run_next_page(request.page_request().clone()).await {');
const pageFailure = runner.indexOf('Err(error) => {', pageMatch);
const cancelCheck = runner.indexOf('if cancel_if_requested(&self.db, &lease).await? {', pageFailure);
const failureDetails = runner.indexOf('let details = replay_failure_details(&error);', cancelCheck);
const finishFailure = runner.indexOf('finish_failure(&self.db, &lease, details).await?', failureDetails);
if (
  pageMatch < 0 ||
  pageFailure <= pageMatch ||
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
  '`index_replay_checkpoint_commit_timeout`',
  'does **not** prove that the database operation was rolled back or cancelled',
  'does not execute a synthetic rollback, rewind or checkpoint write',
  'user cancellation that won the race remains `Cancelled`',
  '`retryable: true`',
  '`StopHandle` interruption remains a separate safe-point-only',
  'not a guarantee that a whole page fits inside a job lease',
]);

console.log('[verify-index-replay-pending-future-timeout] replay mutation/checkpoint storage futures are bounded with retryable timeout identity while lease, cancel, and graceful-stop semantics stay separate');
