#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-dead-letter-inspection] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const inspectorPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_dead_letter_inspector.rs';
const postgresPath = 'crates/rustok-index/src/infrastructure/postgres/mod.rs';
const admissionPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs';
const serverPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const docsPath = 'crates/rustok-index/docs/m6-reconciliation-dead-letter-inspection.md';
const docsIndexPath = 'crates/rustok-index/docs/README.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const inspector = requireMarkers(inspectorPath, [
  'const RECONCILIATION_FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";',
  'const MAX_ERROR_CODE_BYTES: usize = 128;',
  'pub struct IndexReconciliationDeadLetterInspection',
  'job_id: Uuid,',
  'attempt_count: u32,',
  'error_code: Option<String>,',
  'dependency_code: String,',
  'retryable: bool,',
  '#[serde(deny_unknown_fields)]',
  'struct StoredReconciliationFailure',
  'pub struct PostgresIndexReconciliationDeadLetterInspector',
  'pub async fn inspect(',
  'IndexReconciliationDeadLetterInspectionError::NilTenantId',
  'IndexReconciliationDeadLetterInspectionError::NilJobId',
  'row.map(|row| decode_failed_job(&row, job_id))',
  'inspection_is_tenant_scoped_and_bounded',
  'inspection_fails_closed_on_unbounded_diagnostic_shape',
]);

const production = inspector.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'tokio::time::sleep',
  'std::thread::sleep',
  'INSERT INTO index_jobs',
  'UPDATE index_jobs',
  'DELETE FROM index_jobs',
  "state = 'pending'",
  'requeue',
  'retry_epoch',
  'Router::new',
  'async_graphql',
]) {
  if (production.includes(forbidden)) {
    fail(`${inspectorPath} production boundary contains forbidden marker ${forbidden}`);
  }
}

const selectStart = production.indexOf('fn select_failed_job_sql(');
const selectEnd = production.indexOf('\n#[derive(Debug, Error', selectStart);
if (selectStart < 0 || selectEnd <= selectStart) {
  fail(`${inspectorPath} must retain one bounded failed-job SELECT`);
}
const select = production.slice(selectStart, selectEnd);
for (const marker of [
  'SELECT {attempt_count} AS attempt_count_value, last_error_code, last_error_details',
  'FROM index_jobs',
  'tenant_id = {prefix}1',
  'job_id = {prefix}2',
  "kind = 'reconcile'",
  "state = 'failed'",
  'LIMIT 1',
]) {
  if (!select.includes(marker)) fail(`${inspectorPath} SELECT is missing ${marker}`);
}
for (const forbidden of [
  'SELECT *',
  'request',
  'cursor',
  'source_name',
  'worker_id',
  'lease_owner',
  'lease_expires_at',
  'completed_at',
  'module_name',
  'entity_name',
]) {
  if (select.includes(forbidden)) {
    fail(`${inspectorPath} SELECT contains forbidden field ${forbidden}`);
  }
}

const decodeStart = production.indexOf('fn decode_failed_job(');
const decodeEnd = production.indexOf('\nfn validate_machine_code(', decodeStart);
if (decodeStart < 0 || decodeEnd <= decodeStart) {
  fail(`${inspectorPath} stored dead-letter decoder is missing`);
}
const decode = production.slice(decodeStart, decodeEnd);
for (const marker of [
  'u32::try_from(attempt_count)',
  'if attempt_count == 0',
  'validate_machine_code(code)',
  '.try_get("", "last_error_details")',
  'serde_json::from_value(details)',
  'details.contract != RECONCILIATION_FAILURE_CONTRACT',
  'validate_machine_code(&details.dependency_code)',
]) {
  if (!decode.includes(marker)) fail(`${inspectorPath} decoder is missing ${marker}`);
}

const machineCodeStart = production.indexOf('fn validate_machine_code(');
const backendStart = production.indexOf('\nfn ensure_supported_backend(', machineCodeStart);
const machineCode = production.slice(machineCodeStart, backendStart);
for (const marker of [
  'value.is_empty()',
  'value.len() > MAX_ERROR_CODE_BYTES',
  'value.trim() != value',
  'byte.is_ascii_lowercase()',
  'byte.is_ascii_digit()',
  "matches!(byte, b'.' | b'_' | b'-')",
]) {
  if (!machineCode.includes(marker)) fail(`${inspectorPath} machine-code guard is missing ${marker}`);
}

for (const marker of [
  '.map_err(|_| IndexReconciliationDeadLetterInspectionError::Storage)?;',
  '#[error("Index reconciliation dead-letter inspection storage operation failed")]',
  'Storage,',
]) {
  if (!production.includes(marker)) fail(`${inspectorPath} stable storage error is missing ${marker}`);
}
if (production.includes('Storage(String)') || production.includes('Storage(#[source]')) {
  fail(`${inspectorPath} storage error must not retain database details`);
}

requireMarkers(postgresPath, [
  'mod source_reconciliation_dead_letter_inspector;',
  'pub use source_reconciliation_dead_letter_inspector::{',
  'IndexReconciliationDeadLetterInspection, IndexReconciliationDeadLetterInspectionError,',
  'PostgresIndexReconciliationDeadLetterInspector,',
  'mod source_reconciliation_runner;',
  'mod source_replay_retry;',
  'PostgresIndexReconciliationRunner,',
  'PostgresIndexReplayRetryStore,',
]);

const admission = requireMarkers(admissionPath, [
  'IndexReconciliationRunError::DeadLettered',
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  'SELECT job_id, state, request, cursor, last_error_code',
]);
const admissionSelectStart = admission.indexOf('fn select_jobs_sql(');
const admissionSelectEnd = admission.indexOf('\nfn insert_job_sql(', admissionSelectStart);
const admissionSelect = admission.slice(admissionSelectStart, admissionSelectEnd);
if (admissionSelect.includes('last_error_details')) {
  fail(`${admissionPath} ordinary admission must still exclude last_error_details`);
}

const server = requireMarkers(serverPath, [
  'Permission::MODULES_MANAGE',
  'pub async fn run(',
  'pub async fn request_cancel(',
  'PostgresIndexReconciliationRunner::new',
]);
const serverProduction = server.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'PostgresIndexReconciliationDeadLetterInspector',
  'IndexReconciliationDeadLetterInspection',
  'inspect_dead_letter',
  'pub async fn inspect(',
]) {
  if (serverProduction.includes(forbidden)) {
    fail(`${serverPath} prematurely exposes reconciliation inspection via ${forbidden}`);
  }
}

requireMarkers(docsPath, [
  'Status: `source_complete_authorized_composition_pending`.',
  'Unlike ordinary dead-letter admission, inspection deliberately reads `last_error_details`',
  'The raw JSON object is never returned.',
  'requires the request-scoped effective `modules:manage` permission',
  'currently exposes only guarded `run` and `request_cancel` operations',
  'manual requeue or retry-epoch reset',
  'canonical M6 drift-diagnosis and targeted-repair roadmap item remains open',
  'maintainer-run',
]);
requireMarkers(docsIndexPath, [
  '[M6 Reconciliation Dead-letter Admission](./m6-reconciliation-dead-letter-admission.md)',
  '[M6 Reconciliation Dead-letter Inspection](./m6-reconciliation-dead-letter-inspection.md)',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers(aggregatePath, [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-reconciliation-dead-letter-admission.mjs'",
  "'verify-index-reconciliation-dead-letter-inspection.mjs'",
  "'verify-index-replay-dead-letter-admission.mjs'",
]);

console.log('[verify-index-reconciliation-dead-letter-inspection] OK');
