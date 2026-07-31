import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_source_failed_progress_recovery_postgres_test.rs",
  source:
    "crates/rustok-index/tests/reconciliation_source_failed_progress_recovery/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_source_failed_progress_recovery/runner.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_source_failed_progress_recovery/evidence.rs",
  database:
    "crates/rustok-index/tests/reconciliation_source_failed_progress_recovery/database.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-source-failed-progress-recovery-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation source failed recovery file: ${relative}`);
  }
  return fs.readFileSync(absolute, "utf8");
};

const requireMarkers = (name, markers) => {
  const content = read(name);
  for (const marker of markers) {
    if (!content.includes(marker)) {
      throw new Error(`${files[name]} is missing required marker: ${marker}`);
    }
  }
};

const forbidMarkers = (name, markers) => {
  const content = read(name);
  for (const marker of markers) {
    if (content.includes(marker)) {
      throw new Error(`${files[name]} contains forbidden marker: ${marker}`);
    }
  }
};

requireMarkers("target", [
  "retryable_source_failure_preserves_progress_and_recovers_by_duplicate_redelivery",
  "IndexReconciliationRunError::Source",
  "IndexSourceError::SourceFailure",
  "IndexSourceFailureKind::Retryable",
  'const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed"',
  'const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1"',
  "failed.pages_processed, 1",
  'failed.source_cursor, json!({ "offset": 1 })',
  '"retryable": true',
  "IndexReconciliationTerminalState::Failed",
  "assert_ne!(recovery_job_id, failed.job_id)",
  "recovery.heartbeat_count(), 1",
  "recovery.applied_count(), 1",
  "recovery.duplicate_count(), 1",
  "recovery.stale_count(), 0",
  "IndexReconciliationRunStatus::AlreadyComplete",
  'count(&inspection, "index_jobs")',
  'count(&inspection, "index_entities")',
  'count(&inspection, "index_inbox")',
]);

requireMarkers("source", [
  'pub const FAILURE_CODE: &str = "owner_source_retryable_after_progress"',
  "FailSecondPage",
  "RecoverSecondPage",
  'IndexSourceCursor::new(json!({ "offset": 1 }))',
  "valid_mutation(request.tenant_id(), 801, 15_801)",
  "valid_mutation(request.tenant_id(), 802, 15_802)",
  "IndexSourceFailure::retryable(FAILURE_CODE)",
  'FieldName::new("id")',
  "source_version: 1",
]);

requireMarkers("runner", [
  "source-failed-progress-recovery-primary",
  "PostgresIndexReconciliationRunner::new",
  "Duration::from_secs(3_600)",
  "        4,",
  "        1,",
]);

requireMarkers("evidence", [
  "cursor->'source_cursor' AS source_cursor",
  "last_error_code",
  "last_error_details",
  "lease_released",
  "completed_at IS NOT NULL",
  "ORDER BY created_at DESC LIMIT 1",
]);

requireMarkers("database", [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "CREATE SCHEMA",
  "IndexModule.migrations()",
  "persist_schema",
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("docs", [
  "Status: executable target retained, not run.",
  "retryable failure after one safe page boundary",
  "exact three-field diagnostic",
  "one applied, one duplicate and zero stale",
  "No sleep, polling delay, elapsed-time expiry or concurrent race is used.",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log(
  "Index reconciliation source failed progress recovery harness markers verified.",
);
