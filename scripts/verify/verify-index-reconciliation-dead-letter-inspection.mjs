import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  inspector:
    "crates/rustok-index/src/infrastructure/postgres/source_reconciliation_dead_letter_inspector.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-dead-letter-inspection.md",
  audit: "crates/rustok-index/docs/implementation-plan-audit-2026-07-31.md",
  postgres: "crates/rustok-index/src/infrastructure/postgres/mod.rs",
  lib: "crates/rustok-index/src/lib.rs",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation dead-letter inspection file: ${relative}`);
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
  return content;
};

const inspector = requireMarkers("inspector", [
  "pub struct IndexReconciliationDeadLetterInspection",
  "pub struct PostgresIndexReconciliationDeadLetterInspector",
  "pub async fn inspect(",
  "kind = 'reconcile' AND state = 'failed'",
  "index_reconciliation_run_failure_v1",
  "#[serde(deny_unknown_fields)]",
  "validate_machine_code",
  "IndexReconciliationDeadLetterInspectionError::Storage",
  "inspection_is_tenant_scoped_and_bounded",
  "inspection_fails_closed_on_unbounded_diagnostic_shape",
]);

for (const forbidden of [
  "SELECT *",
  "request, cursor",
  "lease_owner",
  "worker_id",
  "completed_at",
  "format!(\"Index reconciliation dead-letter inspection storage operation failed:",
  "tokio::spawn",
  "UPDATE index_jobs",
  "DELETE FROM index_jobs",
]) {
  if (inspector.includes(forbidden)) {
    throw new Error(`${files.inspector} contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers("docs", [
  "Status: `source_complete_owner_execution_pending`",
  "modules:manage",
  "actor/reason audit records",
  "manual requeue or retry-epoch reset",
  "did not run",
]);
requireMarkers("audit", [
  "merged_main",
  "open_source_complete",
  "owner_evidence_pending",
  "PR #2743",
  "PR #2639",
  "PR #2642",
  "PR #2644",
  "PR #2693",
]);
requireMarkers("postgres", [
  "mod source_reconciliation_dead_letter_inspector;",
  "PostgresIndexReconciliationDeadLetterInspector",
]);
requireMarkers("lib", [
  "IndexReconciliationDeadLetterInspection",
  "PostgresIndexReconciliationDeadLetterInspector",
]);

console.log("Index reconciliation dead-letter inspection contract verified.");
