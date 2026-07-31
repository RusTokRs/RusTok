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
  authorize: "crates/rustok-payment/src/checkout_execution/prepare_authorize.rs",
  capture: "crates/rustok-payment/src/checkout_execution/capture_provider.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-local-persistence-diagnostic-safety-source.json",
  reasonEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-local-persistence-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const validation = read(paths.validation);
const helpers = read(paths.helpers);
const authorizeSource = read(paths.authorize);
const captureSource = read(paths.capture);
const authorize = functionBody(authorizeSource, "authorize");
const capture = functionBody(captureSource, "capture");
const localSites = `${authorize}\n${capture}`;
const evidence = JSON.parse(read(paths.evidence));
const reasonEvidence = JSON.parse(read(paths.reasonEvidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const [body, operation, reason, label] of [
  [authorize, "authorize", "AuthorizationLocalPersistenceIncomplete", paths.authorize],
  [capture, "capture", "CaptureLocalPersistenceIncomplete", paths.capture],
]) {
  for (const marker of [
    "Err(error) => {",
    "self.mark_local_persistence_failed(",
    `"${operation}",`,
    "let context_facts = checkout_payment_execution_context_facts(context);",
    "let error_facts = checkout_payment_execution_payment_error_facts(&error);",
    "operation_id_non_nil = !journaled.operation_id.is_nil()",
    `provider_operation = "${operation}"`,
    "local_persistence_error_variant = error_facts.error_variant",
    "local_persistence_error_text_field_count = error_facts.text_field_count",
    "local_persistence_error_text_total_length = error_facts.text_total_length",
    "local_persistence_error_uuid_field_count = error_facts.uuid_field_count",
    "local_persistence_error_uuid_non_nil_count = error_facts.uuid_non_nil_count",
    "local_persistence_error_opaque_payload_present = error_facts.opaque_payload_present",
    'code = "payment.checkout_execution_local_persistence_failed"',
    `CheckoutPaymentExecutionReconciliationReason::${reason}`,
  ]) {
    requireText(body, marker, `${label}: local persistence contract`);
  }
  for (const forbidden of [
    "error = ?error",
    "error = %error",
    "error.to_string()",
    "format!(\"{error",
    "local_persistence_database_error =",
    "local_persistence_validation_error =",
    "local_persistence_provider_id =",
    "local_persistence_uuid =",
  ]) {
    forbidText(body, forbidden, `${label}: local persistence payload logging`);
  }
  requireOrder(
    body,
    [
      "self.mark_local_persistence_failed(",
      "let error_facts = checkout_payment_execution_payment_error_facts(&error);",
      'code = "payment.checkout_execution_local_persistence_failed"',
      `CheckoutPaymentExecutionReconciliationReason::${reason}`,
    ],
    `${label}: checkpoint/log/reconciliation order`,
  );
}

requireCount(
  localSites,
  'code = "payment.checkout_execution_local_persistence_failed"',
  2,
  "local persistence diagnostic site count",
);
requireCount(
  localSites,
  "checkout_payment_execution_payment_error_facts(&error)",
  2,
  "safe PaymentError fact extraction count",
);

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
for (const forbidden of ["format!(", ".to_string()", "database_error ="]) {
  forbidText(facts, forbidden, `${paths.validation}: PaymentError payload values`);
}

const reconciliation = functionBody(validation, "manual_reconciliation");
for (const marker of [
  "reason: CheckoutPaymentExecutionReconciliationReason",
  "reconciliation_reason,",
  "PortError::new(",
  "PortErrorKind::Conflict",
  '"payment.checkout_execution_manual_reconciliation"',
  '"payment checkout execution requires manual reconciliation"',
  "false,",
]) {
  requireText(reconciliation, marker, `${paths.validation}: public reconciliation envelope`);
}

for (const marker of [
  'code = "payment.checkout_execution_commit_checkpoint_failed"',
  'code = "payment.checkout_execution_reconciliation_checkpoint_failed"',
  "error = ?error",
]) {
  requireText(helpers, marker, `${paths.helpers}: checkpoint diagnostics remain separate`);
}
for (const marker of [
  'code = "payment.provider_request_encoding_failed"',
  'code = "payment.checkout_execution_provider_failure_checkpoint_failed"',
  'code = "payment.provider_result_encoding_failed"',
  'code = "payment.checkout_execution_provider_checkpoint_failed"',
  "error = ?checkpoint_error",
]) {
  requireText(captureSource, marker, `${paths.capture}: provider checkpoint slice remains open`);
}

if (
  evidence.status !==
  "payment_checkout_execution_local_persistence_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
const expectedContract = {
  local_persistence_diagnostic_site_count: 2,
  authorization_site_sanitized: true,
  capture_site_sanitized: true,
  complete_payment_error_logged: false,
  database_error_text_logged: false,
  validation_text_logged: false,
  uuid_value_logged: false,
  provider_payload_text_logged: false,
  static_error_variant_logged: true,
  aggregate_text_shape_logged: true,
  aggregate_uuid_shape_logged: true,
  opaque_payload_presence_logged: true,
  stable_local_persistence_code_preserved: true,
  operation_identity_shape_preserved: true,
  bounded_context_shape_preserved: true,
  mark_local_persistence_failed_call_order_preserved: true,
  authorization_manual_reconciliation_route_preserved: true,
  capture_manual_reconciliation_route_preserved: true,
  public_manual_reconciliation_envelope_preserved: true,
  typed_reconciliation_reason_variant_count: 16,
  all_checkout_execution_reconciliation_call_sites_typed: true,
  provider_checkpoint_diagnostics_changed: false,
  request_result_encoding_diagnostics_changed: false,
  remaining_provider_checkpoint_diagnostics_open: true,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  journal_mutation_changed: false,
  broad_ecommerce_cleanup_closed: false,
};
for (const [key, expected] of Object.entries(expectedContract)) {
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

if (reasonEvidence.source_contract?.reconciliation_reason_variant_count !== 16) {
  failures.push(`${paths.reasonEvidence}: reconciliation_reason_variant_count must be 16`);
}
if (reasonEvidence.source_contract?.all_checkout_execution_call_sites_typed !== true) {
  failures.push(`${paths.reasonEvidence}: all checkout execution call sites must be typed`);
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "They do not record the complete `PaymentError`", `${paths.doc}: payload policy`);
requireText(doc, "Separate cleanup remains open for provider checkpoint", `${paths.doc}: remaining work`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution local-persistence diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution authorization and capture local-persistence diagnostics retain only stable PaymentError shape facts; checkpoint and runtime evidence remain open",
);
