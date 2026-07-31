import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target: "crates/rustok-index/tests/source_reconciliation_heartbeat_takeover_postgres_test.rs",
  source: "crates/rustok-index/tests/reconciliation_heartbeat_takeover/source.rs",
  runner: "crates/rustok-index/tests/reconciliation_heartbeat_takeover/runner.rs",
  job: "crates/rustok-index/tests/reconciliation_heartbeat_takeover/job.rs",
  database: "crates/rustok-index/tests/reconciliation_heartbeat_takeover/database.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-heartbeat-takeover-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation heartbeat takeover file: ${relative}`);
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
  "heartbeat_blocks_takeover_until_exact_lease_expiry",
  "Barrier::new(2)",
  "shorten_attempt_one",
  "after_heartbeat.lease_extended",
  "IndexReconciliationRunStatus::Busy",
  "expire_attempt_one",
  "takeover.attempt_count(), Some(2)",
  "IndexReconciliationRunError::LeaseLost",
  "assert_eq!(attempt_count, 1)",
  "assert_eq!(final_job.pages_processed, 2)",
  "count(&inspection, \"index_jobs\")",
  "count(&inspection, \"index_entities\")",
  "count(&inspection, \"index_inbox\")",
]);

requireMarkers("source", [
  "Self::Blocking",
  "first_entered.wait().await",
  "second_entered.wait().await",
  "IndexSourceCursor::new",
  "event: u128",
  "source_version: 1",
]);

requireMarkers("runner", [
  "heartbeat-takeover-primary",
  "heartbeat-worker",
  "Duration::from_secs(3_600)",
  "        2,",
  "        1,",
]);

requireMarkers("job", [
  "CURRENT_TIMESTAMP + INTERVAL '30 minutes'",
  "CURRENT_TIMESTAMP + INTERVAL '50 minutes'",
  "CURRENT_TIMESTAMP - INTERVAL '1 second'",
  "lease_owner = 'heartbeat-worker-a'",
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
  "No sleep, polling delay, or wall-clock race is used.",
  "a competing invocation returns `Busy`",
  "Worker A returns `LeaseLost` for attempt 1",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation heartbeat takeover harness markers verified.");
