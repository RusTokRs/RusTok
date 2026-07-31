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
  source: "crates/rustok-payment/src/checkout_compensation_context.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json",
  review:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json",
  doc: "crates/rustok-payment/docs/checkout-compensation-local-context.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const plan = read(paths.plan);
const helper = functionBody(source, "checkout_payment_compensation_port_error_facts");
const mapper = functionBody(source, "map_checkout_payment_compensation_local_port_error");

for (const marker of [
  "struct CheckoutPaymentCompensationPortErrorFacts {",
  "error_kind: &'static str",
  "message_present: bool",
  "message_length: usize",
  "fn checkout_payment_compensation_port_error_facts(",
]) {
  requireText(source, marker, `${paths.source}: error fact model`);
}

for (const [variant, label] of [
  ["Validation", "validation"],
  ["NotFound", "not_found"],
  ["Conflict", "conflict"],
  ["Forbidden", "forbidden"],
  ["Unavailable", "unavailable"],
  ["Timeout", "timeout"],
  ["InvariantViolation", "invariant_violation"],
]) {
  requireText(
    helper,
    `PortErrorKind::${variant} => "${label}"`,
    `${paths.source}: static ${variant} label`,
  );
}
for (const marker of [
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
]) {
  requireText(helper, marker, `${paths.source}: message shape`);
}
for (const forbidden of ["format!(", ".to_string()", "message_text", "debug_error"]) {
  forbidText(helper, forbidden, `${paths.source}: fact payload values`);
}

for (const marker of [
  "checkout_payment_compensation_local_operation(error.code.as_str())",
  '"require_manual_reconciliation" | "validate_provider_journal_state"',
  "PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation",
  "let context_facts = checkout_payment_compensation_context_facts(context);",
  "let error_facts = checkout_payment_compensation_port_error_facts(&error);",
  "tracing::error!(",
  "tracing::warn!(",
  "internal_code = %error.code",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "retryable = error.retryable",
  "boundary = PAYMENT_COMPENSATION_BOUNDARY",
  "\n    error\n}",
]) {
  requireText(mapper, marker, `${paths.source}: wrapper mapper`);
}

requireCount(mapper, "error_message_present = error_facts.message_present", 2, "two message presence fields");
requireCount(mapper, "error_message_length = error_facts.message_length", 2, "two message length fields");
requireCount(mapper, "error_kind = error_facts.error_kind", 2, "two static kind fields");
requireCount(mapper, "internal_code = %error.code", 2, "two stable code fields");

for (const forbidden of [
  "error = ?error",
  "error = %error",
  "internal_message",
  "error.message",
  "error_kind = ?error.kind",
  "error_kind = %error.kind",
  "format!(\"{error",
  "error.to_string()",
]) {
  forbidText(mapper, forbidden, `${paths.source}: complete PortError payload`);
}

for (const marker of [
  "tenant_id_length = context_facts.tenant_id_length",
  "actor_kind = context_facts.actor_kind",
  "checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil",
  "collection_id_present = facts.collection_id_present",
  "reason_length = ?facts.reason_length",
  "metadata_kind = facts.metadata_kind",
  "correlation_id = %context.correlation_id",
]) {
  requireText(mapper, marker, `${paths.source}: retained safe context`);
}

if (
  evidence.status !==
  "payment_checkout_compensation_wrapper_diagnostic_safety_source_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  stable_code_only_local_classification: true,
  human_message_control_flow: false,
  complete_port_error_logged_by_wrapper: false,
  port_error_message_text_logged_by_wrapper: false,
  static_error_kind_logged_by_wrapper: true,
  error_message_presence_logged_by_wrapper: true,
  error_message_length_logged_by_wrapper: true,
  retryability_logged_by_wrapper: true,
  delegated_port_error_changed: false,
  same_port_error_returned: true,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  severity_classification_changed: false,
  local_operation_mapping_changed: false,
  owner_delegation_changed: false,
  request_response_dto_changed: false,
  persistent_owner_source_changed: false,
  persistent_owner_diagnostic_cleanup_complete: false,
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
  "provider_replay_proven",
  "restart_proven",
  "remote_port_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

if (
  review.status !==
  "payment_checkout_compensation_wrapper_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}
for (const [key, expected] of Object.entries({
  same_port_error_returned: true,
  complete_port_error_logging_removed: true,
  port_error_message_text_logging_removed: true,
  static_error_kind_logged: true,
  error_message_shape_logged: true,
  severity_classification_preserved: true,
  local_operation_mapping_preserved: true,
  persistent_owner_source_unchanged: true,
  persistent_owner_diagnostic_cleanup_complete: false,
  focused_wrapper_guard_added: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "They no longer record the complete `PortError`",
  "The same original `PortError` is returned",
  "Persistent owner payload-shape cleanup remains open.",
  "No FBA or FFA status is promoted from source inspection.",
]) {
  requireText(doc, marker, `${paths.doc}: wrapper error policy`);
}
requireText(
  plan,
  "Finish correlation-safe mapper cleanup for order, payment execution/compensation,",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout compensation wrapper error diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout compensation wrapper diagnostics retain only stable PortError kind and message shape while returning the same public error; owner payload cleanup remains open",
);
