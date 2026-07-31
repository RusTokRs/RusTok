import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_mutation_failure_diagnostics_postgres_test.rs",
  source:
    "crates/rustok-index/tests/reconciliation_mutation_failure_diagnostics/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_mutation_failure_diagnostics/runner.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_mutation_failure_diagnostics/evidence.rs",
  database:
    "crates/rustok-index/tests/reconciliation_mutation_failure_diagnostics/database.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-mutation-failure-diagnostics-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation mutation failure file: ${relative}`);
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
  "invalid_mutation_terminalizes_with_permanent_bounded_diagnostic",
  "storage_failure_terminalizes_with_retryable_bounded_diagnostic",
  "IndexReconciliationRunError::MutationFailed",
  "IndexReplayFailureKind::Permanent",
  "IndexReplayFailureKind::Retryable",
  'const PERMANENT_CODE: &str = "mutation_rejected"',
  'const RETRYABLE_CODE: &str = "mutation_storage_retryable"',
  'const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed"',
  'const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1"',
  ".last_error_details",
  ".len(),\n        3",
  "hide_entities_table",
  "restore_entities_table",
  "read_running",
  "IndexReconciliationTerminalState::Failed",
  'count(&evidence_db, "index_entities")',
  'count(&evidence_db, "index_inbox")',
  'count(&inspection, "index_entities")',
  'count(&inspection, "index_inbox")',
]);

requireMarkers("source", [
  "MutationFailureSource::",
  "InvalidRecord",
  "BlockingValid",
  "entered.wait().await",
  "release.wait().await",
  "fields: BTreeMap::new()",
  'FieldName::new("id")',
  "source_version: 1",
]);

requireMarkers("runner", [
  "mutation-failure-diagnostics-primary",
  "PostgresIndexReconciliationRunner::new",
  "Duration::from_secs(60)",
]);

requireMarkers("evidence", [
  "last_error_code",
  "last_error_details",
  "lease_released",
  "completed_at IS NOT NULL",
  "state = 'running'",
  "state = 'failed'",
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
  "mutation_rejected",
  "mutation_storage_retryable",
  "temporarily renames only `index_entities`",
  "exact three-field JSON",
  "no database error text",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation mutation failure diagnostics markers verified.");
