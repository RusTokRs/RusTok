import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const files = {
  target: "crates/rustok-index/tests/source_reconciliation_failure_diagnostics_postgres_test.rs",
  source: "crates/rustok-index/tests/reconciliation_failure_diagnostics/source.rs",
  evidence: "crates/rustok-index/tests/reconciliation_failure_diagnostics/evidence.rs",
  database: "crates/rustok-index/tests/reconciliation_failure_diagnostics/database.rs",
  docs: "crates/rustok-index/docs/m6-reconciliation-failure-diagnostics-postgres-harness.md",
};

const read = (name) => {
  const relative = files[name];
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) {
    throw new Error(`missing reconciliation failure diagnostics file: ${relative}`);
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
  "permanent_source_failure_terminalizes_with_bounded_diagnostics",
  "retryable_source_failure_terminalizes_with_retryable_diagnostic",
  "IndexReconciliationRunError::Source(IndexSourceError::SourceFailure",
  "IndexSourceFailureKind::Permanent",
  "IndexSourceFailureKind::Retryable",
  "index.reconciliation_page_failed",
  "index_reconciliation_run_failure_v1",
  "failure.last_error_details",
  ".len(),\n        3",
  "IndexReconciliationCancelOutcome::AlreadyTerminal",
  "IndexReconciliationTerminalState::Failed",
  "count(&evidence_db, \"index_jobs\")",
  "count(&evidence_db, \"index_entities\")",
  "count(&evidence_db, \"index_inbox\")",
]);

requireMarkers("source", [
  "FailureMode::Permanent",
  "FailureMode::Retryable",
  "IndexSourceFailure::permanent",
  "IndexSourceFailure::retryable",
  "Err(self.failure())",
]);

requireMarkers("evidence", [
  "cursor->>'completed_passes'",
  "cursor->>'pages_processed'",
  "last_error_code",
  "last_error_details",
  "lease_owner IS NULL AND lease_expires_at IS NULL",
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
  "The object must contain exactly those three fields.",
  "does not claim redaction of an unbounded source-detail field",
  "terminalizes retryable and permanent page failures identically as `failed`",
  "The canonical M6 reconciliation and drift-repair item therefore remains open.",
]);

forbidMarkers("target", ["tokio::time::sleep", "std::thread::sleep"]);
forbidMarkers("source", ["tokio::time::sleep", "std::thread::sleep"]);

console.log("Index reconciliation failure diagnostics harness markers verified.");
