#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-replay-retry-store] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const retryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_retry.rs';
const retryStore = requireMarkers(retryPath, [
  'pub struct IndexReplayRetryPolicy',
  'Self::new(5, Duration::from_secs(5), Duration::from_secs(300))',
  'pub enum IndexReplayRetryDisposition',
  'RetryScheduled',
  'TerminalPermanent',
  'TerminalExhausted',
  'pub struct PostgresIndexReplayRetryStore',
  'pub async fn record_failure(',
  'const RETRY_DETAILS_CONTRACT: &str = "index_replay_retry_v1";',
  'state = \'pending\'',
  'available_at = {available_at}',
  'state = \'failed\'',
  'lease_owner = {prefix}3',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'cancel_requested = FALSE',
  'default_policy_uses_bounded_exponential_backoff',
  'permanent_failure_is_terminal_on_the_current_attempt',
  'retry_sql_is_lease_fenced_and_preserves_existing_job_identity',
]);
for (const forbidden of [
  'INSERT INTO index_jobs',
  'Storage(String)',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'tracing::',
  'backtrace',
  'stack_trace',
  'tenant_details',
  'request_details',
]) {
  if (retryStore.includes(forbidden)) {
    fail(`${retryPath} contains forbidden scheduler/raw-detail/new-job marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_replay_retry;',
  'IndexReplayRetryDisposition',
  'IndexReplayRetryError',
  'IndexReplayRetryFailure',
  'IndexReplayRetryFailureKind',
  'IndexReplayRetryPolicy',
  'PostgresIndexReplayRetryStore',
]);

requireMarkers('crates/rustok-index/docs/m6-replay-retry-transition-store.md', [
  'Status: `source_complete_runner_wiring_pending`',
  'maximum attempts: `5`',
  'running -> pending',
  'running -> failed',
  '`index_replay_retry_v1`',
  'wiring retry classification from `PostgresIndexReplayRunner` into this store',
  'scope-level dead-letter blocking and an authorized operator requeue command',
  'combined implementation-plan item for retry/backoff,',
  'maintainer-run',
]);

console.log('[verify-index-replay-retry-store] OK');
