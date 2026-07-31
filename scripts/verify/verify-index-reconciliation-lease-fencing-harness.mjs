import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target: "crates/rustok-index/tests/source_reconciliation_lease_fencing_postgres_test.rs",
  source: "crates/rustok-index/tests/reconciliation_lease_fencing/source.rs",
  job: "crates/rustok-index/tests/reconciliation_lease_fencing/job.rs",
  database: "crates/rustok-index/tests/reconciliation_lease_fencing/database.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-lease-fencing-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation lease-fencing file: ${relative}`);
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
  "expired_lease_takeover_fences_stale_reconciliation_worker",
  "Barrier::new(2)",
  "expire_attempt_one",
  "lease-worker-a",
  "lease-worker-b",
  "IndexReconciliationRunError::LeaseLost",
  "assert_eq!(attempt_count, 1)",
  "assert_eq!(takeover.attempt_count(), Some(2))",
  "assert_eq!(final_job.state, \"succeeded\")",
  "count(&inspection, \"index_jobs\")",
  "count(&inspection, \"index_entities\")",
  "count(&inspection, \"index_inbox\")",
]);

requireMarkers("source", [
  "Self::Blocking",
  "entered.wait().await",
  "release.wait().await",
  "event_id: Uuid::from_u128(10_300)",
  "source_version: 1",
]);

requireMarkers("job", [
  "lease_expires_at = CURRENT_TIMESTAMP - INTERVAL '1 second'",
  "lease_owner = 'lease-worker-a'",
  "attempt_count = 1",
  "cursor->>'completed_passes'",
  "cursor->>'pages_processed'",
  "lease_owner IS NULL",
]);

requireMarkers("database", [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "CREATE SCHEMA",
  "IndexModule.migrations()",
  "INSERT INTO index_schemas",
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("docs", [
  "Status: executable target retained, not run.",
  "No sleep, polling delay, or wall-clock race is used.",
  "IndexReconciliationRunError::LeaseLost",
  "does not prove automatic lease scheduling",
  "does not close the combined M6 reconciliation item",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation lease-fencing harness markers verified.");
