import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_schema_admission_postgres_test.rs",
  source: "crates/rustok-index/tests/reconciliation_schema_admission/source.rs",
  runner: "crates/rustok-index/tests/reconciliation_schema_admission/runner.rs",
  evidence: "crates/rustok-index/tests/reconciliation_schema_admission/evidence.rs",
  database: "crates/rustok-index/tests/reconciliation_schema_admission/database.rs",
  schema: "crates/rustok-index/tests/reconciliation_schema_admission/schema.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-schema-admission-postgres-harness.md",
  production:
    "crates/rustok-index/src/infrastructure/postgres/source_reconciliation_runner.rs",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation schema admission file: ${relative}`);
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
  "schema_admission_blocks_jobs_and_preserves_pending_resume_identity",
  "IndexReconciliationRunError::SchemaNotRegistered",
  "IndexReconciliationRunError::SchemaRetired",
  'request(tenant_id, "schema-missing-worker", 1)',
  'count(&inspection, "index_jobs").await?, 0',
  "source.scan_count(), 0",
  "yielded.status(), IndexReconciliationRunStatus::Yielded",
  'pending.source_cursor, json!({ "offset": 1 })',
  'set_schema_status(&inspection, tenant_id, "retired")',
  "retired schema must block pending job claim",
  "still_pending.attempt_count, 1",
  'set_schema_status(&inspection, tenant_id, "active")',
  "resumed.job_id(), Some(job_id)",
  "resumed.attempt_count(), Some(2)",
  "succeeded.pages_processed, 2",
  "retired schema must block terminal completion lookup",
  "IndexReconciliationRunStatus::AlreadyComplete",
  "source.scan_count(), 2",
]);

requireMarkers("source", [
  "AtomicUsize",
  "scan_count",
  "fetch_add(1, Ordering::SeqCst)",
  'IndexSourceCursor::new(json!({ "offset": 1 }))',
  "mutation(request.tenant_id(), 701, 14_701)",
  "mutation(request.tenant_id(), 702, 14_702)",
  'FieldName::new("id")',
  "source_version: 1",
]);

requireMarkers("runner", [
  "schema-admission-primary",
  "PostgresIndexReconciliationRunner::new",
  "max_pages: usize",
  "Duration::from_secs(3_600)",
]);

requireMarkers("evidence", [
  "attempt_count::bigint AS attempt_count_value",
  "cursor->'source_cursor' AS source_cursor",
  "lease_released",
  "completed_at IS NOT NULL",
  "cancel_requested",
  "last_error_code",
  "last_error_details",
]);

requireMarkers("database", [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "CREATE SCHEMA",
  "IndexModule.migrations()",
  "DROP SCHEMA IF EXISTS",
]);
forbidMarkers("database", ["persist_schema"]);

requireMarkers("schema", [
  "persist_schema",
  "set_schema_status",
  'matches!(status, "active" | "retired")',
  "UPDATE index_schemas SET status = $5",
  "assert_eq!(updated.rows_affected(), 1)",
]);

requireMarkers("docs", [
  "verifies the exact tenant/module/entity/version row",
  "an absent persisted schema returns `SchemaNotRegistered`",
  "A new invocation must return `SchemaRetired` before claiming the pending job",
  "the same job UUID",
  "Retired completed scope",
  "No sleep, polling, wall-clock expiry, or concurrent race is used.",
  "The canonical M6 reconciliation and drift-repair item remains open.",
]);

requireMarkers("production", [
  "lock_reconciliation_scope(transaction, request, backend).await?;",
  "verify_schema_registration(transaction, request, backend).await?;",
  "select_jobs_sql(backend)",
  "SchemaNotRegistered(request.schema.clone())",
  "IndexReconciliationRunError::SchemaRetired",
]);

const production = read("production");
const lockIndex = production.indexOf(
  "lock_reconciliation_scope(transaction, request, backend).await?;",
);
const verifyIndex = production.indexOf(
  "verify_schema_registration(transaction, request, backend).await?;",
);
const selectIndex = production.indexOf("select_jobs_sql(backend)", verifyIndex);
if (!(lockIndex >= 0 && lockIndex < verifyIndex && verifyIndex < selectIndex)) {
  throw new Error(
    "reconciliation schema admission must remain after the advisory lock and before job selection",
  );
}

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation schema admission harness markers verified.");
