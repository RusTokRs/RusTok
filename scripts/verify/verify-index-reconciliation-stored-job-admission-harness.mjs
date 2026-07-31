import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_stored_job_admission_postgres_test.rs",
  database:
    "crates/rustok-index/tests/reconciliation_stored_job_admission/database.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_stored_job_admission/evidence.rs",
  source:
    "crates/rustok-index/tests/reconciliation_stored_job_admission/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_stored_job_admission/runner.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-stored-job-admission-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation stored-job admission file: ${relative}`);
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
  "stored_request_mismatch_blocks_claim_and_recovers_after_repair",
  "stored_cursor_contract_blocks_claim_and_recovers_after_repair",
  "IndexReconciliationRunError::InvalidStoredJob",
  "stored reconciliation request does not match the source/pass contract",
  "cursor contract is invalid",
  "assert_pending_boundary(&blocked",
  "calls.load(Ordering::SeqCst), 1",
  "recovery.attempt_count(), Some(2)",
  "recovery.pages_processed(), 1",
  "succeeded.pages_processed, 2",
  "IndexReconciliationRunStatus::AlreadyComplete",
  'count(&inspection, "index_jobs")',
  'count(&inspection, "index_entities")',
  'count(&inspection, "index_inbox")',
]);

requireMarkers("database", [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "IndexModule.migrations()",
  "jsonb_set(request, '{pass_count}', '2'::jsonb, false)",
  "jsonb_set(request, '{pass_count}', '1'::jsonb, false)",
  "index_reconciliation_cursor_corrupt",
  "index_reconciliation_cursor_v1",
  "rows_affected() != 1",
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("evidence", [
  "request->>'pass_count'",
  "cursor->>'contract' AS cursor_contract",
  "cursor->'source_cursor' AS source_cursor",
  "lease_released",
  "completed_at IS NOT NULL",
  "ORDER BY created_at DESC LIMIT 1",
]);

requireMarkers("source", [
  "AtomicUsize",
  "fetch_add(1, Ordering::SeqCst)",
  'IndexSourceCursor::new(json!({ "offset": 1 }))',
  "valid_mutation(request.tenant_id(), 1_101, 16_101)",
  "valid_mutation(request.tenant_id(), 1_102, 16_102)",
  'FieldName::new("id")',
]);

requireMarkers("runner", [
  "stored-job-admission-primary",
  "PostgresIndexReconciliationRunner::new",
  "Duration::from_secs(3_600)",
  "        max_pages,",
  "        1,",
]);

requireMarkers("docs", [
  "Status: executable target retained, not run.",
  "fail-closed admission boundary",
  "before the pending row is claimed",
  "before claim, attempt increment, source scan or any new entity/inbox write",
  "claim the same pending job UUID",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("database", ["DELETE FROM index_jobs", "INSERT INTO index_jobs"]);

console.log("Index reconciliation stored-job admission harness markers verified.");
