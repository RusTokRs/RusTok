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
  diagnostics:
    "crates/rustok-payment/src/checkout_execution/diagnostic_safety.rs",
  validationErrors:
    "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-admission-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-admission-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const diagnostics = read(paths.diagnostics);
const validationErrors = read(paths.validationErrors);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  "struct CheckoutPaymentExecutionPortErrorFacts",
  "fn checkout_payment_execution_port_error_facts(",
  'PortErrorKind::Validation => "validation"',
  'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"',
  'PortErrorKind::InvariantViolation => "invariant_violation"',
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
]) {
  requireText(diagnostics, marker, `${paths.diagnostics}: shared PortError shape policy`);
}

const readAdmission = functionBody(
  validationErrors,
  "require_checkout_payment_read_admission",
);
for (const marker of [
  "require_policy(PortCallPolicy::read())",
  ".inspect_err(|error|",
  '"policy"',
  "log_checkout_payment_execution_admission_rejection(",
]) {
  requireText(readAdmission, marker, `${paths.validationErrors}: read admission`);
}

const writeAdmission = functionBody(
  validationErrors,
  "require_checkout_payment_write_admission",
);
for (const marker of [
  "require_policy(PortCallPolicy::write())",
  "context.require_write_semantics().inspect_err(|error|",
  '"policy"',
  '"write_semantics"',
]) {
  requireText(writeAdmission, marker, `${paths.validationErrors}: write admission`);
}
requireCount(
  writeAdmission,
  "log_checkout_payment_execution_admission_rejection(",
  2,
  `${paths.validationErrors}: write policy and semantics diagnostics`,
);

const logger = functionBody(
  validationErrors,
  "log_checkout_payment_execution_admission_rejection",
);
for (const marker of [
  "let technical_failure = matches!(",
  "PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation",
  "let context_facts = checkout_payment_execution_context_facts(context);",
  "let error_facts = checkout_payment_execution_port_error_facts(error);",
  "tracing::error!(",
  "tracing::warn!(",
  'owner = "rustok_payment"',
  "operation = owner_operation",
  "admission",
  "correlation_id = %context.correlation_id",
  "internal_code = %error.code",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "retryable = error.retryable",
  "boundary = PAYMENT_EXECUTION_BOUNDARY",
]) {
  requireText(logger, marker, `${paths.validationErrors}: safe admission logger`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "internal_message = %error.message",
  "internal_message = ?error.message",
  "message = %error.message",
  "message = ?error.message",
  "error_kind = ?error.kind",
  "error_kind = %error.kind",
  "error.to_string()",
]) {
  forbidText(logger, forbidden, `${paths.validationErrors}: complete admission error diagnostics`);
}
for (const marker of [
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "internal_code = %error.code",
  "retryable = error.retryable",
]) {
  requireCount(logger, marker, 2, `${paths.validationErrors}: warning and error ${marker}`);
}

if (
  evidence.status !==
  "payment_checkout_execution_admission_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  read_admission_preserved: true,
  write_policy_admission_preserved: true,
  write_semantics_admission_preserved: true,
  complete_port_error_logged: false,
  port_error_message_text_logged: false,
  stable_error_code_logged: true,
  static_error_kind_logged: true,
  retryability_logged: true,
  message_shape_only: true,
  context_shape_preserved: true,
  severity_classification_preserved: true,
  original_admission_error_returned: true,
  public_port_error_contract_changed: false,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  owner_payment_error_diagnostics_changed: false,
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
  "It does not record the complete `PortError` or its message text.",
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
    "Payment checkout execution admission diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution admission diagnostics retain only stable code, static kind, retryability, and message shape; execution evidence remains open",
);
