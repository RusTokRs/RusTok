// The operator fence, the entry-point metrics and the page bound of the Forum reconciliation reports
// live in one place: `ReconciliationExecutor`. The guards used to require those lines inside every
// report, so they stayed red after the reports moved onto the executor and nothing checked that the
// move kept the published metric labels - it did not: the entry-point and module-error series were
// renamed to the `forum.`-prefixed span operation.
//
// This module states the contract once for the guards of every report: the executor owns the rules,
// and a report reaches them only through its own `EXECUTOR` constant with its own labels.

import { containsMarker, productionRegion } from "./forum-source-verification.mjs";

export const RECONCILIATION_EXECUTOR_PATH =
  "crates/modules/rustok-forum/src/services/reconciliation_executor.rs";

const EXECUTOR_MARKERS = [
  "pub struct ReconciliationExecutor",
  "pub operation: &'static str",
  "pub entrypoint: &'static str",
  "pub error_type: &'static str",
  "requested.unwrap_or(self.default_limit)",
  ".clamp(1, self.max_limit)",
  "enforce_scope(security, Resource::ForumCategories, Action::Manage)?",
  "enforce_scope(security, Resource::ForumTopics, Action::Manage)?",
  'record_module_entrypoint_call( "forum", self.entrypoint, "library", )',
  "match self.enforce_operator(security) { Ok(()) => f().await, Err(e) => Err(e), }",
  "record_span_duration( self.operation,",
  'record_span_error(self.operation, "owner_report")',
  'record_module_error("forum", self.error_type, "error")',
];

// A report that keeps any of these has a second copy of a rule the executor owns.
const REPORT_FORBIDDEN = [
  "fn enforce_operations_scope",
  "enforce_scope(",
  "record_module_entrypoint_call(",
  "record_span_duration(",
  "record_module_error(",
  ".clamp(",
];

/** Findings for the executor source itself; empty when the contract holds. */
export function reconciliationExecutorFindings(executorSource) {
  return EXECUTOR_MARKERS.filter((marker) => !containsMarker(executorSource, marker)).map(
    (marker) => `Forum reconciliation executor missing ${marker}`,
  );
}

/**
 * Findings for one report. `name` is the report's short name (`counter`, `mention`, ...): the
 * entry-point label is `<name>_reconciliation_report` and the module-error label is
 * `<name>_reconciliation`, the two series the report published before the executor existed.
 */
export function reconciliationReportFindings(serviceSource, name) {
  const production = productionRegion(serviceSource);
  const required = [
    "const EXECUTOR: ReconciliationExecutor = ReconciliationExecutor {",
    `entrypoint: "${name}_reconciliation_report",`,
    `error_type: "${name}_reconciliation",`,
    "EXECUTOR.execute(security, || async {",
    "EXECUTOR.effective_limit(requested_limit)",
  ];
  const findings = required
    .filter((marker) => !containsMarker(production, marker))
    .map((marker) => `Forum ${name} reconciliation service missing ${marker}`);
  for (const forbidden of REPORT_FORBIDDEN) {
    if (containsMarker(production, forbidden)) {
      findings.push(
        `Forum ${name} reconciliation service must leave ${forbidden} to ReconciliationExecutor`,
      );
    }
  }
  return findings;
}

/** Throwing form for the guards, which report the first failure. */
export function assertReconciliationReport({ executorSource, serviceSource, name }) {
  const findings = [
    ...reconciliationExecutorFindings(executorSource),
    ...reconciliationReportFindings(serviceSource, name),
  ];
  if (findings.length > 0) throw new Error(findings.join("\n"));
}
