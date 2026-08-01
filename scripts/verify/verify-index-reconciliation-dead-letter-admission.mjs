import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  runner:
    "crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs",
  target:
    "crates/rustok-index/tests/source_reconciliation_dead_letter_admission_postgres_test.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-dead-letter-admission.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation dead-letter admission file: ${relative}`);
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

requireMarkers("runner", [
  "const MAX_ERROR_CODE_BYTES: usize = 128;",
  "DeadLettered {",
  "job_id: Uuid,",
  "attempt_count: u32,",
  "error_code: Option<String>,",
  '"failed" => {',
  "error_code: stored.last_error_code,",
  "last_error_code: Option<String>,",
  '.try_get("", "last_error_code")',
  "last_error_code is outside the reconciliation error contract",
  "state IN ('pending', 'running', 'succeeded', 'failed')",
  "CASE state WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END",
]);

const runner = read("runner");
const selectStart = runner.indexOf("fn select_jobs_sql");
const selectEnd = runner.indexOf("fn insert_job_sql", selectStart);
if (selectStart < 0 || selectEnd < 0) {
  throw new Error("could not isolate reconciliation job selection SQL");
}
const selectJobs = runner.slice(selectStart, selectEnd);
if (!selectJobs.includes("last_error_code")) {
  throw new Error("reconciliation job selection must load the bounded error code");
}
if (selectJobs.includes("last_error_details")) {
  throw new Error("reconciliation dead-letter admission must not load error details");
}

const succeededBranch = runner.indexOf('"succeeded" => {');
const activeBranch = runner.indexOf('"running" | "pending" if !stored.claimable');
const failedBranch = runner.indexOf('"failed" => {');
if (!(succeededBranch >= 0 && activeBranch > succeededBranch && failedBranch > activeBranch)) {
  throw new Error("reconciliation scope precedence must remain succeeded, active, then failed");
}

requireMarkers("target", [
  "failed_reconciliation_scope_blocks_new_jobs_without_exposing_details",
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "IndexModule.migrations()",
  "owner_source_permanent_dead_letter",
  "IndexReconciliationRunError::Source",
  "IndexReconciliationRunError::DeadLettered",
  "private-reconciliation-failure-detail",
  "assert!(!debug.contains(PRIVATE_MARKER))",
  "assert!(!debug.contains(DEPENDENCY_CODE))",
  "assert!(!display.contains(PRIVATE_MARKER))",
  "assert!(!display.contains(DEPENDENCY_CODE))",
  "assert_eq!(calls.load(Ordering::SeqCst), 1)",
  'count(&evidence_db, "index_jobs")',
  'count(&evidence_db, "index_entities")',
  'count(&evidence_db, "index_inbox")',
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("docs", [
  "Status: production admission and PostgreSQL regression retained, not run.",
  "deterministic precedence",
  "The acquisition query does not load `last_error_details`.",
  "must not call the source, create another job, change the failed row",
  "authorized dead-letter inspection",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", [
  "tokio::time::sleep",
  "std::thread::sleep",
  "DELETE FROM index_jobs",
  "INSERT INTO index_jobs",
]);
forbidMarkers("docs", [
  "implementation-plan.md",
  "automatic requeue is implemented",
  "complete drift repair is implemented",
]);

console.log("Index reconciliation dead-letter admission markers verified.");
