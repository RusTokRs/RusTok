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

const paths = {
  source: "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-reconciliation-reason-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  "enum CheckoutPaymentExecutionReconciliationReason {",
  "MissingNormalizedDurableResult,",
  "MalformedDurableResult,",
  "InvalidSuccessfulProviderResponse,",
  "UnknownProviderOutcome,",
  "impl CheckoutPaymentExecutionReconciliationReason {",
  "fn label(self) -> &'static str",
  'Self::MissingNormalizedDurableResult => "missing_normalized_durable_result"',
  'Self::MalformedDurableResult => "malformed_durable_result"',
  'Self::InvalidSuccessfulProviderResponse => "invalid_successful_provider_response"',
  'Self::UnknownProviderOutcome => "unknown_provider_outcome"',
]) {
  requireText(source, marker, `${paths.source}: closed reconciliation reason policy`);
}

const helper = functionBody(source, "manual_reconciliation");
for (const marker of [
  "reason: CheckoutPaymentExecutionReconciliationReason",
  "let reconciliation_reason = reason.label();",
  "reconciliation_reason,",
  "correlation_id = %context.correlation_id",
  "operation = owner_operation",
  'code = "payment.checkout_execution_manual_reconciliation"',
  "boundary = PAYMENT_EXECUTION_BOUNDARY",
  "PortError::new(",
  "PortErrorKind::Conflict",
  '"payment.checkout_execution_manual_reconciliation"',
  '"payment checkout execution requires manual reconciliation"',
  "false,",
]) {
  requireText(helper, marker, `${paths.source}: reconciliation helper`);
}
for (const forbidden of [
  "internal_message",
  "reason: &'static str",
  "reason: &str",
  "reason = %",
  "reason = ?",
]) {
  forbidText(helper, forbidden, `${paths.source}: free-form reason diagnostics`);
}

for (const marker of [
  "CheckoutPaymentExecutionReconciliationReason::MissingNormalizedDurableResult",
  "CheckoutPaymentExecutionReconciliationReason::MalformedDurableResult",
  "CheckoutPaymentExecutionReconciliationReason::InvalidSuccessfulProviderResponse",
  "CheckoutPaymentExecutionReconciliationReason::UnknownProviderOutcome",
]) {
  requireText(source, marker, `${paths.source}: typed reconciliation call site`);
  requireCount(source, marker, 1, `${paths.source}: one ${marker} call site`);
}
requireCount(
  source,
  "manual_reconciliation(",
  5,
  `${paths.source}: helper plus four call sites`,
);

for (const forbidden of [
  '"payment provider operation has no normalized durable result",',
  '"payment provider operation result is malformed",\n        )',
  '"payment provider returned an invalid successful response",',
  '"payment provider operation outcome is unknown",',
]) {
  forbidText(source, forbidden, `${paths.source}: free-form call-site reasons`);
}

const persisted = functionBody(source, "persisted_provider_result");
for (const marker of [
  "operation.status == PROVIDER_OPERATION_EXECUTING",
  "PROVIDER_OPERATION_COMMITTED",
  "PROVIDER_OPERATION_SUCCEEDED",
  "PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  "operation.provider_result.clone().ok_or_else(||",
  "serde_json::from_value(value).map(Some).map_err(|_|",
  "CheckoutPaymentExecutionReconciliationReason::MissingNormalizedDurableResult",
  "CheckoutPaymentExecutionReconciliationReason::MalformedDurableResult",
]) {
  requireText(persisted, marker, `${paths.source}: preserved durable result routing`);
}

const ownerMapper = functionBody(source, "payment_error_to_port_error");
for (const marker of [
  "PaymentError::ProviderInvalidResponse { .. } => manual_reconciliation(",
  "CheckoutPaymentExecutionReconciliationReason::InvalidSuccessfulProviderResponse",
  "PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(",
  "CheckoutPaymentExecutionReconciliationReason::UnknownProviderOutcome",
]) {
  requireText(ownerMapper, marker, `${paths.source}: preserved owner reconciliation routing`);
}

if (
  evidence.status !==
  "payment_checkout_execution_reconciliation_reason_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  reconciliation_reason_variant_count: 4,
  free_form_reason_parameter_allowed: false,
  free_form_reason_text_logged: false,
  stable_reason_label_logged: true,
  missing_durable_result_reason_preserved: true,
  malformed_durable_result_reason_preserved: true,
  invalid_successful_response_reason_preserved: true,
  unknown_provider_outcome_reason_preserved: true,
  manual_reconciliation_code_preserved: true,
  manual_reconciliation_kind_preserved: true,
  manual_reconciliation_message_preserved: true,
  manual_reconciliation_retryability_preserved: true,
  call_routing_preserved: true,
  provider_result_recovery_changed: false,
  provider_execution_changed: false,
  payment_lifecycle_changed: false,
  local_persistence_diagnostics_changed: false,
  provider_checkpoint_diagnostics_changed: false,
  remaining_payment_execution_diagnostics_open: true,
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

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(
  doc,
  "The helper no longer accepts or records free-form reconciliation reason text.",
  `${paths.doc}: diagnostic policy`,
);
requireText(
  doc,
  "Remaining payment execution diagnostics",
  `${paths.doc}: remaining work`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution reconciliation-reason diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution reconciliation diagnostics use only four stable typed reasons; public conflict behavior and execution evidence remain unchanged",
);
