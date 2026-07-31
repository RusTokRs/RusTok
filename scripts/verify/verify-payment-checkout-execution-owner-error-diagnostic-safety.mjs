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
  error: "crates/rustok-payment/src/error.rs",
  mapper: "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-owner-error-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-owner-error-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const errorSource = read(paths.error);
const mapperSource = read(paths.mapper);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  "Validation(String)",
  "PaymentCollectionNotFound(Uuid)",
  "PaymentNotFound(Uuid)",
  "RefundNotFound(Uuid)",
  "InvalidTransition { from: String, to: String }",
  "ProviderUnavailable {",
  "ProviderRejected {",
  "ProviderInvalidResponse {",
  "ProviderOutcomeUnknown {",
  "ProviderConfiguration { provider_id: String }",
  "Database(#[from] DbErr)",
]) {
  requireText(errorSource, marker, `${paths.error}: retained PaymentError variant`);
}

const facts = functionBody(
  mapperSource,
  "checkout_payment_execution_payment_error_facts",
);
for (const marker of [
  "CheckoutPaymentExecutionPaymentErrorFacts",
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
  "value.chars().count()",
  "from.chars().count() + to.chars().count()",
  "provider_id.chars().count() + operation.chars().count()",
  "if id.is_nil() { 0 } else { 1 }",
]) {
  requireText(facts, marker, `${paths.mapper}: owner error shape policy`);
}
for (const forbidden of [
  "format!(",
  ".to_string()",
  "error.to_string()",
  "provider_id =",
  "provider_operation =",
  "database_error =",
]) {
  forbidText(facts, forbidden, `${paths.mapper}: owner payload values`);
}
requireCount(
  facts,
  "if id.is_nil() { 0 } else { 1 }",
  3,
  `${paths.mapper}: three UUID-bearing variants`,
);

const stableCode = functionBody(mapperSource, "stable_payment_error_code");
for (const marker of [
  'PaymentError::Database(_) => "payment.database_unavailable"',
  'PaymentError::Validation(_) => "payment.validation"',
  'PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found"',
  'PaymentError::PaymentNotFound(_) => "payment.payment_not_found"',
  'PaymentError::RefundNotFound(_) => "payment.refund_not_found"',
  'PaymentError::InvalidTransition { .. } => "payment.invalid_transition"',
  'PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable"',
  'PaymentError::ProviderRejected { .. } => "payment.provider_rejected"',
  'PaymentError::ProviderInvalidResponse { .. } => "payment.provider_invalid_response"',
  'PaymentError::ProviderOutcomeUnknown { .. } => "payment.provider_outcome_unknown"',
  'PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured"',
]) {
  requireText(stableCode, marker, `${paths.mapper}: stable owner code`);
}

const mapper = functionBody(mapperSource, "payment_error_to_port_error");
for (const marker of [
  "let code = stable_payment_error_code(&error);",
  "let error_facts = checkout_payment_execution_payment_error_facts(&error);",
  "owner_error_variant = error_facts.error_variant",
  "owner_error_text_field_count = error_facts.text_field_count",
  "owner_error_text_total_length = error_facts.text_total_length",
  "owner_error_uuid_field_count = error_facts.uuid_field_count",
  "owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count",
  "owner_error_opaque_payload_present = error_facts.opaque_payload_present",
  "correlation_id = %context.correlation_id",
  "code,",
  "boundary = PAYMENT_EXECUTION_BOUNDARY",
  '"payment checkout execution owner operation failed"',
]) {
  requireText(mapper, marker, `${paths.mapper}: safe owner mapper diagnostics`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "error.to_string()",
  "provider_id =",
  "provider_operation =",
  "validation_message =",
  "database_error =",
  "collection_id =",
  "payment_id =",
  "refund_id =",
]) {
  forbidText(mapper, forbidden, `${paths.mapper}: complete PaymentError diagnostics`);
}

for (const marker of [
  'PortError::unavailable(\n            "payment.database_unavailable",\n            "payment storage is temporarily unavailable"',
  'PortError::validation(\n            "payment.checkout_execution_validation",\n            "checkout payment request is invalid"',
  'PortError::not_found(\n            "payment.collection_not_found",\n            "payment collection was not found"',
  'PortError::not_found("payment.payment_not_found", "payment was not found")',
  'PortError::not_found("payment.refund_not_found", "refund was not found")',
  'PortError::conflict(\n            "payment.checkout_execution_state_conflict",\n            "payment lifecycle conflicts with checkout execution"',
  'PortError::unavailable(\n            "payment.provider_unavailable",\n            "payment provider is temporarily unavailable"',
  'PortError::conflict(\n            "payment.provider_rejected",\n            "payment provider rejected the requested operation"',
  'PaymentError::ProviderInvalidResponse { .. } => manual_reconciliation(',
  '"payment provider returned an invalid successful response"',
  'PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(',
  '"payment provider operation outcome is unknown"',
  'PortError::invariant_violation(\n            "payment.provider_not_configured",\n            "payment provider is not configured"',
]) {
  requireText(mapper, marker, `${paths.mapper}: preserved public mapping`);
}

if (
  evidence.status !==
  "payment_checkout_execution_owner_error_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  payment_error_variant_count: 11,
  complete_payment_error_logged: false,
  database_error_text_logged: false,
  validation_text_logged: false,
  uuid_value_logged: false,
  provider_id_text_logged: false,
  provider_operation_text_logged: false,
  transition_text_logged: false,
  static_error_variant_logged: true,
  text_field_shape_logged: true,
  uuid_field_shape_logged: true,
  opaque_database_payload_presence_logged: true,
  stable_owner_code_logged: true,
  context_shape_preserved: true,
  public_port_error_mapping_preserved: true,
  manual_reconciliation_routing_preserved: true,
  provider_reconciliation_policy_changed: false,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  uuid_serde_diagnostics_changed: false,
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
  "It does not record the complete `PaymentError`",
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
    "Payment checkout execution owner error diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution owner diagnostics retain only stable variant and payload shape; public mappings and execution evidence remain unchanged",
);
