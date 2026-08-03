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
  'const MAX_REPLAY_ATTEMPTS: u32 = 100;',
  'const MAX_BACKOFF_SECONDS: u64 = 86_400;',
  'const RETRY_DETAILS_CONTRACT: &str = "index_replay_retry_v1";',
  'pub struct IndexReplayRetryPolicy',
  'Self::new(5, Duration::from_secs(5), Duration::from_secs(300))',
  'pub enum IndexReplayRetryDisposition',
  'RetryScheduled',
  'TerminalPermanent',
  'TerminalExhausted',
  'pub struct PostgresIndexReplayRetryStore',
  'pub async fn record_failure(',
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

const productionRetry = retryStore.split('\n#[cfg(test)]')[0];
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
  'mutation_payload',
]) {
  if (productionRetry.includes(forbidden)) {
    fail(`${retryPath} production section contains forbidden scheduler/raw-detail/new-job marker ${forbidden}`);
  }
}

const detailsStart = productionRetry.indexOf('fn retry_details(');
const detailsEnd = productionRetry.indexOf('\nfn validate_backoff(', detailsStart);
if (detailsStart < 0 || detailsEnd <= detailsStart) {
  fail(`${retryPath} must retain one bounded retry_details constructor`);
}
const details = productionRetry.slice(detailsStart, detailsEnd);
for (const marker of [
  '"contract": RETRY_DETAILS_CONTRACT',
  '"dependency_code": failure.code()',
  '"failure_kind":',
  '"attempt_count": lease.attempt_count()',
  '"max_attempts": policy.max_attempts()',
  '"disposition": disposition',
  '"retry_after_seconds": retry_after_seconds',
  '"next_attempt": next_attempt',
]) {
  if (!details.includes(marker)) fail(`${retryPath} retry details are missing ${marker}`);
}
for (const forbidden of [
  'tenant_id()',
  'job_id()',
  'worker_id()',
  'schema()',
  'source_name()',
]) {
  if (details.includes(forbidden)) {
    fail(`${retryPath} retry details expose forbidden identity accessor ${forbidden}`);
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

requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_replay_job.rs', [
  "state = 'pending' AND available_at <= CURRENT_TIMESTAMP",
  'attempt_count = stored.attempt_count.checked_add(1)',
]);

const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_replay_runner.rs';
const runner = requireMarkers(runnerPath, [
  'let details = replay_failure_details(&error);',
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

requireMarkers('crates/rustok-index/docs/m6-replay-retry-transition-store.md', [
  'Status: `source_complete_runner_wiring_pending`',
  'maximum attempts: `5`',
  'running -> pending',
  'running -> failed',
  '`index_replay_retry_v1`',
  'current `PostgresIndexReplayRunner` still terminalizes page failures',
  'scope-level failed-job admission',
  'canonical implementation-plan item',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Replay Retry Transition Store](./m6-replay-retry-transition-store.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-replay-retry-store.mjs'",
]);

console.log('[verify-index-replay-retry-store] OK');
