import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target:
    "crates/rustok-index/tests/source_reconciliation_event_id_contract_postgres_test.rs",
  source:
    "crates/rustok-index/tests/reconciliation_event_id_contract/source.rs",
  runner:
    "crates/rustok-index/tests/reconciliation_event_id_contract/runner.rs",
  evidence:
    "crates/rustok-index/tests/reconciliation_event_id_contract/evidence.rs",
  database:
    "crates/rustok-index/tests/reconciliation_event_id_contract/database.rs",
  docs:
    "crates/rustok-index/docs/m6-reconciliation-event-id-contract-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation event-id contract file: ${relative}`);
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
  "nil_second_event_id_rejects_whole_page_before_mutation_persistence",
  "duplicate_second_event_id_rejects_whole_page_before_mutation_persistence",
  "IndexReconciliationRunError::NilEventId",
  "IndexReconciliationRunError::DuplicateEventId",
  "assert_eq!(position, 1)",
  "assert_eq!(event_id, DUPLICATE_EVENT_ID)",
  'const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed"',
  'const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1"',
  'const DEPENDENCY_CODE: &str = "reconciliation_contract_invalid"',
  "failure.completed_passes, 0",
  "failure.pages_processed, 0",
  "Some(3)",
  'count(&inspection, "index_jobs")',
  'count(&inspection, "index_entities")',
  'count(&inspection, "index_inbox")',
  "IndexReconciliationCancelOutcome::NotFound",
  "IndexReconciliationTerminalState::Failed",
]);

requireMarkers("source", [
  "NilSecond",
  "DuplicateSecond",
  "Uuid::nil()",
  "DUPLICATE_EVENT_ID",
  "valid_mutation(request.tenant_id(), 1_001",
  "valid_mutation(request.tenant_id(), 1_002",
  'FieldName::new("id")',
  "source_version: 1",
  "IndexSourcePage::new",
]);

requireMarkers("runner", [
  "event-id-contract-primary",
  "PostgresIndexReconciliationRunner::new",
  "Duration::from_secs(3_600)",
  "        2,",
  "        1,",
]);

requireMarkers("evidence", [
  "cursor->>'completed_passes'",
  "cursor->>'pages_processed'",
  "last_error_code",
  "last_error_details",
  "lease_released",
  "completed_at IS NOT NULL",
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
  "page-wide event identity preflight",
  "second mutation",
  "exact three-field JSON object",
  "zero `index_entities` rows",
  "zero `index_inbox` rows",
  "No sleep, polling delay, elapsed-time expiry, or concurrent race is used.",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation event-id contract harness markers verified.");
