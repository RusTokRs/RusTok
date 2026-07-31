import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_mutation_storage_failed_progress_recovery_postgres_test.rs",
  source:
    "crates/rustok-index/tests/reconciliation_mutation_storage_failed_progress_recovery/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_mutation_storage_failed_progress_recovery/runner.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_mutation_storage_failed_progress_recovery/evidence.rs",
  database:
    "crates/rustok-index/tests/reconciliation_mutation_storage_failed_progress_recovery/database.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-mutation-storage-failed-progress-recovery-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation mutation storage recovery file: ${relative}`);
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
  "retryable_second_page_storage_failure_preserves_progress_and_recovers_by_duplicate_redelivery",
  "MutationStorageFailedProgressSource::BlockSecondPage",
  "entered.wait().await",
  "running.pages_processed, 1",
  'running.source_cursor, json!({ "offset": 1 })',
  'count(&inspection, "index_entities").await?, 1',
  'count(&inspection, "index_inbox").await?, 1',
  "database.hide_entities_table().await?",
  "database.restore_entities_table().await?",
  "IndexReconciliationRunError::MutationFailed",
  "IndexReplayFailureKind::Retryable",
  'const STORAGE_FAILURE_CODE: &str = "mutation_storage_retryable"',
  'failed.source_cursor, json!({ "offset": 1 })',
  '"retryable": true',
  "IndexReconciliationTerminalState::Failed",
  "assert_ne!(recovery_job_id, failed.job_id)",
  "recovery.heartbeat_count(), 1",
  "recovery.applied_count(), 1",
  "recovery.duplicate_count(), 1",
  "recovery.stale_count(), 0",
  "IndexReconciliationRunStatus::AlreadyComplete",
]);

requireMarkers("source", [
  "BlockSecondPage",
  "RecoverSecondPage",
  "entered.wait().await",
  "release.wait().await",
  'IndexSourceCursor::new(json!({ "offset": 1 }))',
  "valid_mutation(request.tenant_id(), 901, 15_901)",
  "valid_mutation(request.tenant_id(), 902, 15_902)",
  'FieldName::new("id")',
  "source_version: 1",
]);

requireMarkers("runner", [
  "mutation-storage-failed-progress-recovery-primary",
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
  "ALTER TABLE index_entities RENAME TO index_entities_temporarily_unavailable",
  "ALTER TABLE index_entities_temporarily_unavailable RENAME TO index_entities",
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("docs", [
  "Status: executable target retained, not run.",
  "temporarily unavailable on page two",
  "The whole page-two mutation transaction must roll back.",
  "exactly one entity and one inbox row",
  "one duplicate, one applied and zero stale mutations",
  "No sleep, polling delay, elapsed-time expiry or concurrent worker race is used.",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log(
  "Index reconciliation mutation storage failed progress recovery harness markers verified.",
);
