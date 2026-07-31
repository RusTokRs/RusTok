import fs from "node:fs";

const paths = {
  target: "crates/rustok-index/tests/source_reconciliation_running_cancel_postgres_test.rs",
  cancel: "crates/rustok-index/tests/reconciliation_running_cancel/cancel.rs",
  recover: "crates/rustok-index/tests/reconciliation_running_cancel/recover.rs",
  source: "crates/rustok-index/tests/reconciliation_running_cancel/source.rs",
  database: "crates/rustok-index/tests/reconciliation_running_cancel/database.rs",
  doc: "crates/rustok-index/docs/m6-reconciliation-running-cancel-postgres-harness.md",
};

const sources = Object.fromEntries(
  Object.entries(paths).map(([name, path]) => [name, fs.readFileSync(path, "utf8")]),
);
const failures = [];
const requireText = (name, text) => {
  if (!sources[name].includes(text)) failures.push(`${paths[name]} missing ${text}`);
};

for (const marker of [
  "running_cancel_preserves_cursor_and_recovers_by_duplicate_redelivery",
  "cancel::run(&database, &control, &inspection)",
  "recover::run(&database, &control, &inspection, cancelled_job_id)",
]) requireText("target", marker);

for (const marker of [
  "control.entered.wait().await",
  "IndexReconciliationCancelOutcome::NotFound",
  "IndexReconciliationCancelOutcome::Requested",
  "cancel_requested AS value",
  "IndexReconciliationRunStatus::Cancelled",
  "cursor->>'completed_passes'",
  "cursor->>'pages_processed'",
  "SELECT COUNT(*)::bigint AS value FROM index_entities",
  "SELECT COUNT(*)::bigint AS value FROM index_inbox",
]) requireText("cancel", marker);

for (const marker of [
  "IndexReconciliationRunStatus::Complete",
  "assert_ne!(recovered.job_id(), Some(cancelled_job_id))",
  "assert_eq!(recovered.applied_count(), 0)",
  "assert_eq!(recovered.duplicate_count(), 1)",
  "state = 'cancelled'",
  "state = 'succeeded'",
]) requireText("recover", marker);

for (const marker of [
  "block_first.swap(false, Ordering::SeqCst)",
  "self.entered.wait().await",
  "self.release.wait().await",
  "event_id: Uuid::from_u128(10_100)",
  "source_version: 1",
]) requireText("source", marker);

for (const marker of [
  "RUSTOK_INDEX_TEST_DATABASE_URL",
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "prepare(&db, tenant_id).await?",
]) requireText("database", marker);

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "completed_passes = 0",
  "pages_processed = 0",
  "one duplicate and zero newly applied mutations",
  "The canonical M6 reconciliation and drift-repair item remains open.",
  "No test, Cargo command, verifier, PostgreSQL target, workflow, or CI job was executed",
]) requireText("doc", marker);

for (const [name, forbidden] of [
  ["target", "#[ignore]"],
  ["cancel", "UPDATE index_jobs"],
  ["recover", "INSERT INTO index_jobs"],
  ["source", "Uuid::new_v4()"],
]) {
  if (sources[name].includes(forbidden)) failures.push(`${paths[name]} contains ${forbidden}`);
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log("Index reconciliation running-cancel PostgreSQL harness contract is retained; execution remains open");
