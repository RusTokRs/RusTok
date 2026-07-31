#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

function regionFrom(source, marker, label) {
  const index = source.indexOf(marker);
  if (index < 0) {
    failures.push(`${label}: missing region marker ${marker}`);
    return "";
  }
  return source.slice(index);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const paths = {
  validation: "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  helpers: "crates/rustok-payment/src/checkout_execution/provider_helpers.rs",
  capture: "crates/rustok-payment/src/checkout_execution/capture_provider.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-checkpoint-encoding-diagnostic-safety-source.json",
  localEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-local-persistence-diagnostic-safety-source.json",
  reasonEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-checkpoint-encoding-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const validation = read(paths.validation);
const helpers = read(paths.helpers);
const capture = read(paths.capture);
const commitCheckpoint = functionBody(helpers, "mark_journal_committed");
const reconciliationCheckpoint = functionBody(helpers, "mark_local_persistence_failed");
const providerExecution = functionBody(capture, "execute_journaled_provider_operation");
const providerFailureCheckpoint = regionFrom(
  providerExecution,
  "if let Err(checkpoint_error) = checkpoint",
  `${paths.capture}: provider failure checkpoint`,
);
const providerSuccessCheckpoint = regionFrom(
  providerExecution,
  ".mark_provider_succeeded(",
  `${paths.capture}: provider success checkpoint`,
);
const evidence = JSON.parse(read(paths.evidence));
const localEvidence = JSON.parse(read(paths.localEvidence));
const reasonEvidence = JSON.parse(read(paths.reasonEvidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const checkpointSites = [
  [
    commitCheckpoint,
    "commit_checkpoint_error",
    "payment.checkout_execution_commit_checkpoint_failed",
    `${paths.helpers}: commit checkpoint`,
  ],
  [
    reconciliationCheckpoint,
    "reconciliation_checkpoint_error",
    "payment.checkout_execution_reconciliation_checkpoint_failed",
    `${paths.helpers}: reconciliation checkpoint`,
  ],
  [
    providerFailureCheckpoint,
    "provider_failure_checkpoint_error",
    "payment.checkout_execution_provider_failure_checkpoint_failed",
    `${paths.capture}: provider failure checkpoint`,
  ],
  [
    providerSuccessCheckpoint,
    "provider_success_checkpoint_error",
    "payment.checkout_execution_provider_checkpoint_failed",
    `${paths.capture}: provider success checkpoint`,
  ],
];

for (const [body, prefix, code, label] of checkpointSites) {
  requireText(
    body,
    "checkout_payment_execution_payment_error_facts(&",
    `${label}: safe PaymentError fact extraction`,
  );
  for (const field of [
    "error_variant",
    "text_field_count",
    "text_total_length",
    "uuid_field_count",
    "uuid_non_nil_count",
    "opaque_payload_present",
  ]) {
    requireText(body, `${prefix}_${field} = error_facts.${field}`, `${label}: ${field}`);
  }
  requireText(body, `code = "${code}"`, `${label}: stable code`);
  for (const forbidden of [
    "error = ?error",
    "error = %error",
    "error = ?checkpoint_error",
    "error = %checkpoint_error",
    "error.to_string()",
    "checkpoint_error.to_string()",
  ]) {
    forbidText(body, forbidden, `${label}: payload logging`);
  }
}

for (const marker of [
  "serde_json::to_value(&request).map_err(|_|",
  "request_encoding_failed = true",
  'code = "payment.provider_request_encoding_failed"',
  "PortError::invariant_violation(",
  '"payment provider request could not be encoded"',
  "serde_json::to_value(&provider_result).map_err(|_|",
  "result_encoding_failed = true",
  'code = "payment.provider_result_encoding_failed"',
  "CheckoutPaymentExecutionReconciliationReason::ProviderResultEncodingFailed",
]) {
  requireText(providerExecution, marker, `${paths.capture}: encoding contract`);
}
for (const forbidden of ["serde_error =", "serialization_error =", "error = ?error"]) {
  forbidText(providerExecution, forbidden, `${paths.capture}: serde error payload`);
}

requireCount(
  `${helpers}\n${providerExecution}`,
  "checkout_payment_execution_payment_error_facts(&",
  4,
  "four checkpoint PaymentError fact extractions",
);
requireCount(providerExecution, "request_encoding_failed = true", 1, "request encoding flag");
requireCount(providerExecution, "result_encoding_failed = true", 1, "result encoding flag");

requireOrder(
  commitCheckpoint,
  [
    "self.operation_journal.mark_committed(operation_id).await",
    "checkout_payment_execution_payment_error_facts(&error)",
    'code = "payment.checkout_execution_commit_checkpoint_failed"',
    ".mark_reconciliation_required(",
    "CheckoutPaymentExecutionReconciliationReason::CommitCheckpointFailed",
  ],
  `${paths.helpers}: commit checkpoint order`,
);
requireOrder(
  reconciliationCheckpoint,
  [
    ".mark_reconciliation_required(",
    "checkout_payment_execution_payment_error_facts(&error)",
    'code = "payment.checkout_execution_reconciliation_checkpoint_failed"',
  ],
  `${paths.helpers}: reconciliation checkpoint order`,
);
requireOrder(
  providerFailureCheckpoint,
  [
    "if let Err(checkpoint_error) = checkpoint",
    "checkout_payment_execution_payment_error_facts(&checkpoint_error)",
    'code = "payment.checkout_execution_provider_failure_checkpoint_failed"',
    "CheckoutPaymentExecutionReconciliationReason::ProviderFailureCheckpointFailed",
  ],
  `${paths.capture}: provider failure checkpoint order`,
);
requireOrder(
  providerSuccessCheckpoint,
  [
    ".mark_provider_succeeded(",
    ".map_err(|error| {",
    "checkout_payment_execution_payment_error_facts(&error)",
    'code = "payment.checkout_execution_provider_checkpoint_failed"',
    "CheckoutPaymentExecutionReconciliationReason::ProviderSuccessCheckpointFailed",
  ],
  `${paths.capture}: provider success checkpoint order`,
);

for (const marker of [
  "error.requires_provider_reconciliation()",
  ".mark_reconciliation_required(journal_operation.id, code)",
  ".mark_provider_error(journal_operation.id, code)",
  ".mark_provider_succeeded(",
]) {
  requireText(providerExecution, marker, `${paths.capture}: preserved provider journal behavior`);
}

const facts = functionBody(validation, "checkout_payment_execution_payment_error_facts");
for (const marker of [
  '"validation"',
  '"payment_collection_not_found"',
  '"payment_not_found"',
  '"refund_not_found"',
  '"invalid_transition"',
  '"provider_unavailable"',
  '"provider_rejected"',
  '"provider_invalid_response"',
  '"provider_outcome_unknown"',
  '"provider_configuration"',
  'PaymentError::Database(_) => ("database", 0, 0, 0, 0, true)',
]) {
  requireText(facts, marker, `${paths.validation}: retained PaymentError shape mapping`);
}

if (
  evidence.status !==
  "payment_checkout_execution_checkpoint_encoding_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  diagnostic_site_count: 6,
  payment_error_checkpoint_site_count: 4,
  serde_encoding_site_count: 2,
  commit_checkpoint_site_sanitized: true,
  reconciliation_checkpoint_site_sanitized: true,
  provider_failure_checkpoint_site_sanitized: true,
  provider_success_checkpoint_site_sanitized: true,
  provider_request_encoding_site_sanitized: true,
  provider_result_encoding_site_sanitized: true,
  complete_payment_error_logged: false,
  serde_error_text_logged: false,
  static_payment_error_variant_logged: true,
  request_encoding_failure_flag_logged: true,
  result_encoding_failure_flag_logged: true,
  stable_codes_preserved: true,
  journal_mutation_order_preserved: true,
  provider_failure_classification_preserved: true,
  manual_reconciliation_routes_preserved: true,
  request_encoding_public_envelope_preserved: true,
  public_manual_reconciliation_envelope_preserved: true,
  typed_reconciliation_reason_variant_count: 16,
  checkout_execution_payload_diagnostic_cleanup_closed: true,
  remaining_checkout_execution_payload_diagnostics_open: false,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  journal_mutation_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "compile_proven",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const [companionPath, companion] of [
  [paths.localEvidence, localEvidence],
  [paths.reasonEvidence, reasonEvidence],
]) {
  if (companion.source_contract?.provider_checkpoint_diagnostics_changed !== true) {
    failures.push(`${companionPath}: provider checkpoint diagnostics must be closed`);
  }
  if (companion.source_contract?.request_result_encoding_diagnostics_changed !== true) {
    failures.push(`${companionPath}: request/result encoding diagnostics must be closed`);
  }
  if (companion.source_contract?.remaining_provider_checkpoint_diagnostics_open !== false) {
    failures.push(`${companionPath}: remaining provider checkpoint diagnostics must be false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "six private failure events", `${paths.doc}: six-site scope`);
requireText(doc, "They do not record serde error text", `${paths.doc}: serde payload policy`);
requireText(doc, "payload-diagnostic sites at source level", `${paths.doc}: source closure`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution checkpoint/encoding diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution checkpoint and encoding diagnostics retain only bounded PaymentError shape facts or static encoding flags; execution validation remains open",
);
