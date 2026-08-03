#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-source-reconciliation] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const runnerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const runner = requireMarkers(runnerPath, [
  'RECONCILIATION_JOB_REQUEST_CONTRACT: &str = "index_reconciliation_job_v1"',
  'RECONCILIATION_JOB_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1"',
  'const MAX_PAGES_PER_RUN: usize = 1_024;',
  'const MAX_PASSES: u32 = 8;',
  'pub struct IndexReconciliationRunRequest',
  'pub enum IndexReconciliationRunStatus',
  'RetryScheduled,',
  'FailedPermanent,',
  'FailedExhausted,',
  'pub struct IndexReconciliationRunOutcome',
  'pub fn retry_after(&self) -> Option<Duration>',
  'pub fn next_attempt(&self) -> Option<u32>',
  'pub struct PostgresIndexReconciliationRunner',
  '.source_for_schema(request.schema())',
  'IndexSourceScanRequest::new(',
  'self.sources.scan(scan_request).await',
  '.apply_replay_mutation(',
  'state.source_cursor = next_cursor;',
  'state.completed_passes',
  'persist_progress(&self.db, &lease, &state).await?',
  'finish_success(&self.db, &lease, &state).await?',
  'yield_for_resume(&self.db, &lease).await?',
  'cancel_if_requested(&self.db, &lease).await?',
  'PostgresIndexReconciliationRetryStore::new(db.clone())',
  'retry_store.record_failure(&retry_lease, &failure).await',
  "kind = 'reconcile'",
  "scope_kind = 'schema'",
  'cursor = {prefix}5',
  'attempt_count = {prefix}4',
  'lease_expires_at > CURRENT_TIMESTAMP',
  'pg_advisory_xact_lock(hashtextextended($1, 0))',
  '#[serde(deny_unknown_fields)]',
  'IndexReplayMutationOutcome::Duplicate',
  'IndexReplayMutationOutcome::StaleIgnored',
]);
forbidMarkers(runnerPath, runner, [
  'rustok_product',
  'rustok_channel',
  'rustok_search',
  'product_index_tombstones',
  'product_variant_index_tombstones',
  'INSERT INTO index_entities',
  'UPDATE index_entities',
  'DELETE FROM index_entities',
  'async fn finish_failure(',
  'fn finish_failure_sql(',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
]);

const sourceResolution = runner.indexOf('.source_for_schema(request.schema())');
const storedRequest = runner.indexOf('ReconciliationJobRequest {');
if (sourceResolution < 0 || storedRequest <= sourceResolution) {
  fail('the source must be registry-resolved before its identity is stored in the job request');
}
const apply = runner.indexOf('.apply_replay_mutation(');
const progress = runner.indexOf('persist_progress(&self.db, &lease, &state).await?');
if (apply < 0 || progress <= apply) {
  fail('durable job progress must follow mutation persistence');
}

requireMarkers(
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner_tests.rs',
  [
    'two_pass_reconciliation_catches_insert_behind_first_cursor',
    'bounded_reconciliation_yields_and_resumes_durable_pass_state',
    'retryable_failure_schedules_due_attempts_and_terminally_exhausts',
    'permanent_failure_terminalizes_without_retry_metadata',
    'reconciliation_request_bounds_pages_passes_and_heartbeat_cadence',
    'IndexReconciliationRunStatus::AlreadyComplete',
    'IndexReconciliationRunStatus::RetryScheduled',
    'IndexReconciliationRunStatus::FailedExhausted',
    'IndexReconciliationRunStatus::FailedPermanent',
    "json_extract(cursor, '$.completed_passes')",
    "json_extract(cursor, '$.pages_processed')",
    'assert_eq!(outcome.applied_count(), 3);',
    'assert_eq!(outcome.duplicate_count(), 2);',
  ],
);

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod source_reconciliation_retry;',
  'mod source_reconciliation_runner;',
  'mod source_reconciliation_runner_tests;',
  'IndexReconciliationRetryDisposition',
  'PostgresIndexReconciliationRetryStore',
  'IndexReconciliationRunRequest',
  'IndexReconciliationRunOutcome',
  'PostgresIndexReconciliationRunner',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'bounded multi-pass source reconciliation with durable pass/cursor progression',
  'bounded reconciliation retry transitions',
  'IndexReconciliationRunRequest',
  'IndexReconciliationRunStatus',
  'PostgresIndexReconciliationRunner',
]);

const operationsMigration = requireMarkers(
  'crates/rustok-index/src/migrations/m20260727_000003_create_index_operations.rs',
  [
    'ColumnDef::new(IndexJobs::Cursor).json_binary()',
    "kind IN ('schema_apply', 'secondary_index', 'rebuild', 'reconcile', 'consistency_check')",
  ],
);
forbidMarkers(
  'crates/rustok-index/src/migrations/m20260727_000003_create_index_operations.rs',
  operationsMigration,
  ['index_reconciliation_cursor_v1'],
);

requireMarkers('crates/rustok-index/docs/m7-product-reconciliation.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`PostgresIndexReconciliationRunner`',
  '`index_reconciliation_job_v1`',
  '`index_reconciliation_cursor_v1`',
  'two passes are the recommended minimum',
  'narrows but does not eliminate the live-write window',
  'does **not** claim a repeatable-read owner snapshot',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-source-reconciliation.mjs'",
  "'verify-index-reconciliation-runner-retry.mjs'",
]);

console.log('[verify-index-source-reconciliation] OK');
