import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_stale_version_guard_postgres_test.rs",
  database:
    "crates/rustok-index/tests/reconciliation_stale_version_guard/database.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_stale_version_guard/evidence.rs",
  source:
    "crates/rustok-index/tests/reconciliation_stale_version_guard/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_stale_version_guard/runner.rs",
  schema:
    "crates/rustok-index/tests/reconciliation_stale_version_guard/schema.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-stale-version-guard-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation stale-version guard file: ${relative}`);
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
  "reconciliation_retains_stale_delete_and_resurrection_guards",
  "IndexReconciliationRunStatus::Yielded",
  "first.applied_count(), 1",
  "first.stale_count(), 1",
  'pending.source_cursor, json!({ "offset": 2 })',
  "live.source_version, 3",
  "!live.is_deleted",
  "IndexReconciliationRunStatus::Complete",
  "second.attempt_count(), Some(2)",
  "succeeded.pages_processed, 4",
  "succeeded.applied_count, 2",
  "succeeded.stale_count, 2",
  "tombstone.source_version, 4",
  "tombstone.is_deleted",
  "tombstone.payload, None",
  "IndexReconciliationRunStatus::AlreadyComplete",
  "calls.load(Ordering::SeqCst), 4",
]);

requireMarkers("source", [
  "FRESH_UPSERT_EVENT_ID",
  "STALE_DELETE_EVENT_ID",
  "FRESH_DELETE_EVENT_ID",
  "STALE_UPSERT_EVENT_ID",
  "fresh_upsert(request.tenant_id())",
  "stale_delete(request.tenant_id())",
  "fresh_delete(request.tenant_id())",
  "stale_upsert(request.tenant_id())",
  "source_version: 2",
  "source_version: 4",
  "fetch_add(1, Ordering::SeqCst)",
]);

requireMarkers("evidence", [
  "cursor->>'stale_count'",
  "source_version::bigint AS source_version_value",
  "state = 'applied'",
  "completed_at IS NOT NULL",
  "delivery_id = $2",
  '"index_links"',
]);

requireMarkers("runner", [
  "stale-version-guard-primary",
  "PostgresIndexReconciliationRunner::new",
  "Duration::from_secs(3_600)",
  "        max_pages,",
  "        1,",
]);

requireMarkers("schema", [
  "stale-version-guard-harness",
  'FieldName::new("id")',
  'FieldName::new("marker_id")',
  "IndexValueType::Uuid",
  "status) VALUES ($1, $2, $3, $4, $5, $6, 'active')",
]);

requireMarkers("database", [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "IndexModule.migrations()",
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
]);

requireMarkers("docs", [
  "Status: executable target retained, not run.",
  "stale mutation is terminally acknowledged",
  "stale deletion of a live entity",
  "stale resurrection after a newer tombstone",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("database", [
  "INSERT INTO index_jobs",
  "UPDATE index_jobs",
  "DELETE FROM index_jobs",
]);

console.log("Index reconciliation stale-version guard harness markers verified.");
