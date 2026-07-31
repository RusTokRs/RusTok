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
  root: "crates/rustok-payment/src/checkout_execution.rs",
  diagnostics:
    "crates/rustok-payment/src/checkout_execution/diagnostic_safety.rs",
  portImpl: "crates/rustok-payment/src/checkout_execution/port_impl.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-local-porterror-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const rootSource = read(paths.root);
const diagnostics = read(paths.diagnostics);
const portImpl = read(paths.portImpl);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  'include!("checkout_execution/diagnostic_safety.rs");',
  'include!("checkout_execution/port_impl.rs");',
  'const PREPARE_CHECKOUT_COLLECTION_OPERATION: &str = "prepare_checkout_collection"',
  'const AUTHORIZE_CHECKOUT_COLLECTION_OPERATION: &str = "authorize_checkout_collection"',
  'const CAPTURE_CHECKOUT_COLLECTION_OPERATION: &str = "capture_checkout_collection"',
  'const READ_CHECKOUT_COLLECTION_OPERATION: &str = "read_checkout_collection"',
]) {
  requireText(rootSource, marker, `${paths.root}: preserved execution composition`);
}

for (const marker of [
  "struct CheckoutPaymentExecutionPortErrorFacts",
  "error_kind: &'static str",
  "message_present: bool",
  "message_length: usize",
  "fn checkout_payment_execution_port_error_facts(",
  'PortErrorKind::Validation => "validation"',
  'PortErrorKind::NotFound => "not_found"',
  'PortErrorKind::Conflict => "conflict"',
  'PortErrorKind::Forbidden => "forbidden"',
  'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"',
  'PortErrorKind::InvariantViolation => "invariant_violation"',
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
]) {
  requireText(diagnostics, marker, `${paths.diagnostics}: PortError shape policy`);
}

const mapper = functionBody(
  portImpl,
  "map_checkout_payment_execution_local_port_error",
);
for (const marker of [
  "checkout_payment_execution_local_operation(operation, error.code.as_str())",
  "let error_facts = checkout_payment_execution_port_error_facts(&error);",
  "internal_code = %error.code",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "retryable = error.retryable",
  "boundary = PAYMENT_EXECUTION_BOUNDARY",
  '"payment checkout execution local technical outcome retained safe context"',
  '"payment checkout execution local outcome retained safe context"',
]) {
  requireText(mapper, marker, `${paths.portImpl}: safe final local mapper`);
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
  forbidText(mapper, forbidden, `${paths.portImpl}: complete PortError diagnostics`);
}
requireCount(
  mapper,
  "error_message_present = error_facts.message_present",
  2,
  `${paths.portImpl}: warning and error message-presence facts`,
);
requireCount(
  mapper,
  "error_message_length = error_facts.message_length",
  2,
  `${paths.portImpl}: warning and error message-length facts`,
);
requireCount(
  mapper,
  "error_kind = error_facts.error_kind",
  2,
  `${paths.portImpl}: warning and error static kind facts`,
);
requireCount(
  mapper,
  "internal_code = %error.code",
  2,
  `${paths.portImpl}: stable code diagnostics`,
);
requireCount(
  mapper,
  "retryable = error.retryable",
  2,
  `${paths.portImpl}: retry policy diagnostics`,
);
requireText(mapper, "return error;", `${paths.portImpl}: unknown-code passthrough`);
if (!mapper.trimEnd().endsWith("error\n}")) {
  failures.push(`${paths.portImpl}: mapper must return the original PortError`);
}

requireCount(
  portImpl,
  "map_checkout_payment_execution_local_port_error(",
  5,
  `${paths.portImpl}: four callsites plus mapper definition`,
);
requireCount(
  portImpl,
  "result.map_err(|error|",
  4,
  `${paths.portImpl}: all four final mappings`,
);
for (const operation of [
  "prepare_checkout_collection",
  "authorize_checkout_collection",
  "capture_checkout_collection",
  "read_checkout_collection",
]) {
  requireText(portImpl, `async fn ${operation}(`, `${paths.portImpl}: ${operation}`);
}

if (
  evidence.status !==
  "payment_checkout_execution_local_porterror_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  operation_count: 4,
  diagnostic_helper_added: true,
  complete_port_error_logged: false,
  port_error_message_text_logged: false,
  stable_error_code_logged: true,
  static_error_kind_logged: true,
  retryability_logged: true,
  message_shape_only: true,
  context_shape_preserved: true,
  identity_shape_preserved: true,
  original_port_error_returned: true,
  public_port_error_contract_changed: false,
  payment_lifecycle_changed: false,
  provider_execution_changed: false,
  admission_diagnostics_changed: false,
  owner_payment_error_diagnostics_changed: false,
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
  `${paths.doc}: remaining bounded work`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution local PortError diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution final local PortError diagnostics retain only stable code, static kind, retryability, and message shape; execution evidence remains open",
);
