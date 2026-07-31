import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target: "crates/rustok-index/tests/source_reconciliation_heartbeat_cancel_postgres_test.rs",
  source: "crates/rustok-index/tests/reconciliation_heartbeat_cancel/source.rs",
  runner: "crates/rustok-index/tests/reconciliation_heartbeat_cancel/runner.rs",
  job: "crates/rustok-index/tests/reconciliation_heartbeat_cancel/job.rs",
  database: "crates/rustok-index/tests/reconciliation_heartbeat_cancel/database.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-heartbeat-cancel-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation heartbeat cancellation file: ${relative}`);
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
  "cancellation_after_heartbeat_preserves_cursor_and_recovers_duplicates",
  "Barrier::new(2)",
  "shorten_attempt_one",
  "after_heartbeat.lease_extended",
  "IndexReconciliationCancelOutcome::NotFound",
  "IndexReconciliationCancelOutcome::Requested",
  "cancel_requested.lease_extended",
  "IndexReconciliationRunStatus::Cancelled",
  "cancelled_outcome.heartbeat_count(), 1",
  "cancelled_job.pages_processed, 1",
  "IndexReconciliationTerminalState::Cancelled",
  "assert_ne!(recovery_job_id, initial.job_id)",
  "recovery.duplicate_count(), 2",
  "IndexReconciliationRunStatus::AlreadyComplete",
  "count(&inspection, \"index_jobs\")",
  "count(&inspection, \"index_entities\")",
  "count(&inspection, \"index_inbox\")",
]);

requireMarkers("source", [
  "Self::Blocking",
  "first_entered.wait().await",
  "second_entered.wait().await",
  "IndexSourceCursor::new",
  "12_501",
  "12_502",
  "source_version: 1",
]);

requireMarkers("runner", [
  "heartbeat-cancel-primary",
  "Duration::from_secs(3_600)",
  "        2,",
  "        1,",
]);

requireMarkers("job", [
  "CURRENT_TIMESTAMP + INTERVAL '30 minutes'",
  "CURRENT_TIMESTAMP + INTERVAL '50 minutes'",
  "lease_owner = 'heartbeat-cancel-worker-a'",
  "cancel_requested",
  "attempt_count = 1",
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
  "No sleep, polling delay, or elapsed-time race is used.",
  "The exact tenant then requests cancellation",
  "The durable cancelled job must retain the previous safe cursor boundary",
  "report zero newly applied mutations and two duplicates",
  "cancellation while the heartbeat SQL statement itself is concurrently executing",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation heartbeat cancellation harness markers verified.");
