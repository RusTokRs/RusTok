#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-host-scheduler] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const cargoPath = 'crates/modules/rustok-index/Cargo.toml';
const libPath = 'crates/modules/rustok-index/src/lib.rs';
const postgresPath = 'crates/modules/rustok-index/src/infrastructure/postgres/mod.rs';
const schedulerPath =
  'crates/modules/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const runnerPath =
  'crates/modules/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const replayRuntimePath =
  'crates/modules/rustok-index/src/infrastructure/postgres/replay_runtime.rs';
const runtimePath = 'crates/libs/rustok-runtime/src/lib.rs';
const appRuntimePath = 'apps/server/src/services/app_runtime.rs';
const docsPath = 'crates/modules/rustok-index/docs/m6-reconciliation-host-scheduler.md';
const retryDocsPath =
  'crates/modules/rustok-index/docs/m6-reconciliation-retry-transition-store.md';
const runnerDocsPath =
  'crates/modules/rustok-index/docs/m6-reconciliation-runner-retry-wiring.md';
const docsIndexPath = 'crates/modules/rustok-index/docs/README.md';
const planPath = 'crates/modules/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

requireMarkers(cargoPath, [
  'rustok-core.workspace = true',
  'rustok-runtime.workspace = true',
]);

const scheduler = requireMarkers(schedulerPath, [
  'pub const INDEX_RECONCILIATION_WORKER: &str = "index_reconciliation";',
  'const RECONCILIATION_WORK_ITEM_CONTRACT: &str = "index_reconciliation_scheduler_item_v1";',
  'const DEFAULT_PAGE_LIMIT: usize = 100;',
  'const DEFAULT_MAX_PAGES: usize = 8;',
  'const DEFAULT_HEARTBEAT_EVERY_PAGES: usize = 1;',
  'const DEFAULT_LEASE_SECONDS: u64 = 300;',
  'pub struct IndexReconciliationSchedulerPolicy',
  'pub struct PostgresIndexReconciliationWorkAdapter',
  'pub(crate) struct IndexReconciliationWorkRegistration;',
  'impl ModuleWorkRegistration for IndexReconciliationWorkRegistration',
  'impl ModuleWorkSource for PostgresIndexReconciliationWorkAdapter',
  'impl ModuleWorkHandler for PostgresIndexReconciliationWorkAdapter',
  '#[serde(deny_unknown_fields)]',
]);
const production = scheduler.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'std::thread::sleep',
  'loop {',
  'Router::new',
  'async_graphql',
  'reqwest',
  'INSERT INTO index_jobs',
  'UPDATE index_jobs',
  'DELETE FROM index_jobs',
]) {
  if (production.includes(forbidden)) {
    fail(`${schedulerPath} production boundary contains forbidden marker ${forbidden}`);
  }
}

const registrationStart = production.indexOf('impl ModuleWorkRegistration');
const sourceStart = production.indexOf('impl ModuleWorkSource', registrationStart);
const registration = production.slice(registrationStart, sourceStart);
for (const marker of [
  'host.shared_get::<SharedIndexSourceRegistry>()',
  'return Ok(());',
  'host.shared_get::<SharedIndexSchemaRegistry>()',
  'index.reconciliation_scheduler.missing_schema_registry',
  'IndexReconciliationSchedulerPolicy::default()',
  '.register_with(scheduler)',
]) {
  if (!registration.includes(marker)) {
    fail(`${schedulerPath} registration is missing ${marker}`);
  }
}

const claimStart = production.indexOf('    async fn claim(');
const completeStart = production.indexOf('    async fn complete(', claimStart);
const claim = production.slice(claimStart, completeStart);
const slugCheck = claim.indexOf('worker_slug != INDEX_RECONCILIATION_WORKER');
const discovery = claim.indexOf('discover_due_reconciliation(&self.db, &self.sources, &self.schemas).await?');
if (slugCheck < 0 || discovery <= slugCheck) {
  fail(`${schedulerPath} must reject the wrong worker slug before database discovery`);
}

const sqlStart = production.indexOf('fn due_reconciliation_sql(');
const sqlEnd = production.indexOf('\nfn unverified(', sqlStart);
if (sqlStart < 0 || sqlEnd <= sqlStart) fail(`${schedulerPath} read-only due SQL is missing`);
const sql = production.slice(sqlStart, sqlEnd);
for (const marker of [
  'ROW_NUMBER() OVER',
  'PARTITION BY tenant_id, module_name, entity_name, schema_version',
  "WHEN 'succeeded' THEN 0",
  "WHEN 'running' THEN 1",
  "WHEN 'pending' THEN 2",
  "kind = 'reconcile'",
  "scope_kind = 'schema'",
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  'scope_rank = 1 AND {scope} AND available_at <= CURRENT_TIMESTAMP',
  "available_at <= CURRENT_TIMESTAMP",
  "state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP",
  'ORDER BY available_at, created_at, tenant_id, job_id',
  'LIMIT 1',
]) {
  if (!sql.includes(marker)) fail(`${schedulerPath} due discovery SQL is missing ${marker}`);
}
for (const forbidden of ['INSERT ', 'UPDATE ', 'DELETE ', 'last_error_details']) {
  if (sql.includes(forbidden)) fail(`${schedulerPath} due discovery SQL contains ${forbidden}`);
}

const decodeStart = production.indexOf('fn decode_due_work(');
const backendStart = production.indexOf('fn ensure_supported_backend(', decodeStart);
const decode = production.slice(decodeStart, backendStart);
for (const marker of [
  'tenant_id.is_nil() || job_id.is_nil()',
  'schema_version == 0',
  'serde_json::from_value(request_json)',
  'request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT',
  'request.pass_count == 0',
  'sources.source_for_schema(&schema)',
  'source.source_name() != request.source_name',
  'IndexReconciliationRunRequest::new(',
]) {
  if (!decode.includes(marker)) fail(`${schedulerPath} stored work validation is missing ${marker}`);
}

const executeStart = production.indexOf('    async fn execute(');
const discoverStart = production.indexOf('\nasync fn discover_due_reconciliation(', executeStart);
const execute = production.slice(executeStart, discoverStart);
for (const marker of [
  'Self::decode_item(&item)?',
  'format!("index-reconciliation-{}", invocation_id.simple())',
  'self.policy.page_limit()',
  'self.policy.max_pages()',
  'self.policy.heartbeat_every_pages()',
  'self.policy.lease_duration()',
  '.runner',
  '.run(request)',
  'IndexReconciliationRunStatus::Cancelled => ModuleWorkOutcome::Cancelled',
  'IndexReconciliationRunStatus::Busy',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
  'ModuleWorkOutcome::Completed',
  'for_discovered_job(',
  'defer_due_job(',
]) {
  if (!execute.includes(marker)) fail(`${schedulerPath} handler is missing ${marker}`);
}
for (const forbidden of ['format!("{error', 'last_error_details']) {
  if (execute.includes(forbidden)) fail(`${schedulerPath} handler exposes ${forbidden}`);
}

const complete = production.slice(completeStart, production.indexOf('\n}\n\n#[async_trait]', completeStart));
for (const marker of [
  'discovered.verifies_outcome(persisted, &state, outcome)',
  "WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND scope_kind = 'schema'",
  'request.pass_count != payload.pass_count',
  'cursor.completed_passes != request.pass_count || cursor.source_cursor.is_some()',
]) {
  if (!complete.includes(marker)) fail(`${schedulerPath} completion fence is missing ${marker}`);
}
for (const forbidden of ['.execute_raw(', 'defer_due_job(', 'INSERT ', 'UPDATE ', 'DELETE ']) {
  if (complete.includes(forbidden)) fail(`${schedulerPath} completion must be read-only: ${forbidden}`);
}

requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/source_job_discovery.rs', [
  'self.generation == i64::from(generation) && self.retry_epoch == retry_epoch',
  'let same_epoch = persisted.retry_epoch == self.retry_epoch;',
  'let acquired = self.generation.checked_add(1) == Some(persisted.generation);',
  'state == "succeeded" && same_epoch && (acquired || persisted == self)',
  'ModuleWorkOutcome::Cancelled => state == "cancelled" && same_epoch && acquired',
  'ModuleWorkOutcome::Rejected { .. } => state == "failed" && same_epoch && acquired',
  'sources.source_for_schema(reference)',
  "s.status = 'active'",
  'AND s.schema_fingerprint = ${}',
  'AND {source_name} = ${}',
  'AND attempt_count = {prefix}3 AND retry_epoch = {prefix}4',
  'due_job_where(backend, kind)',
  'deadline_after_seconds(backend, 6)',
]);
requireMarkers('crates/modules/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests/hardening.rs', [
  'postgres_concurrent_reconciliation_requeue_keeps_generation_and_one_immutable_audit',
  'adapter.claim(INDEX_RECONCILIATION_WORKER)',
  'adapter.complete(&item, result)',
  '&stale("retry_epoch", 2)',
  '&stale("lease_generation", 4)',
]);

requireMarkers(runnerPath, [
  'lock_reconciliation_scope(transaction, request, backend).await?;',
  'claim_job_sql(backend)',
  'attempt_count = {prefix}4',
  'IndexReconciliationRunStatus::RetryScheduled',
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
]);

requireMarkers(postgresPath, [
  'mod source_reconciliation_scheduler;',
  'pub use source_reconciliation_scheduler::{',
  'INDEX_RECONCILIATION_WORKER,',
  'IndexReconciliationSchedulerPolicy,',
  'PostgresIndexReconciliationWorkAdapter,',
  'pub(crate) use source_reconciliation_scheduler::IndexReconciliationWorkRegistration;',
]);
requireMarkers(libPath, [
  'host-owned due reconciliation scheduling through the generic module-work lifecycle',
  'IndexReconciliationSchedulerPolicy',
  'PostgresIndexReconciliationWorkAdapter',
  'INDEX_RECONCILIATION_WORKER',
  'register_postgres_index_reconciliation_work',
  'IndexReconciliationSchedulerCompositionError',
]);

requireMarkers(replayRuntimePath, [
  'ReconciliationScheduler(#[from] IndexReconciliationSchedulerCompositionError)',
  'register_postgres_index_reconciliation_work(extensions)?;',
  'publishes neither a false replay capability',
  'nor an empty Index module-work registration',
  'assert!(!extensions.contains::<ModuleWorkRegistrations>());',
  'assert!(extensions.contains::<ModuleWorkRegistrations>());',
]);

requireMarkers(runtimePath, [
  'pub struct ModuleWorkScheduler',
  'pub async fn run_once(&self)',
  'pub async fn run_until_stopped(',
  'A stop prevents future claims',
  'work already claimed by an',
  'adapter is allowed to finish its canonical completion path',
]);
requireMarkers(appRuntimePath, [
  'initialize_module_work_runtime',
  'ModuleWorkScheduler::new()',
  '.register_all(&host, &scheduler)',
  'runs_background_workers()',
  'StopHandle',
  '.run_until_stopped(stop, std::time::Duration::from_secs(1))',
]);

requireMarkers(docsPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'The generic host scheduler remains the only polling and lifecycle owner',
  'Only rank one may become work',
  'The discovery SELECT is read-only and is not lease acquisition.',
  'original generation, epoch and due state match',
  'outcomes require the same retry epoch and the exact lease generation',
  'Multiple hosts may discover the same due row',
  'The canonical retry/backoff/dead-letter/global-scheduling plan item remains open',
  'maintainer-run',
]);
requireMarkers(retryDocsPath, [
  'Status: `host_scheduler_source_complete_owner_execution_pending`.',
  'generic host `ModuleWorkScheduler`',
  'Fleet duplicates remain safe',
]);
requireMarkers(runnerDocsPath, [
  'Status: `host_scheduler_source_complete_owner_execution_pending`.',
  'The generic `ModuleWorkScheduler` owns polling cadence and graceful StopHandle shutdown',
  'only one pending or expired-running attempt can acquire',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Host Scheduler](./m6-reconciliation-host-scheduler.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-reconciliation-retry-store.mjs'",
  "'verify-index-reconciliation-runner-retry.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-reconciliation-host-scheduler] OK');
