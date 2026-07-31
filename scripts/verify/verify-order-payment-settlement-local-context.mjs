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

const paths = {
  wrapper: "crates/rustok-order/src/checkout_owner_context.rs",
  owner: "crates/rustok-order/src/checkout_payment_settlement.rs",
  doc: "crates/rustok-order/docs/checkout-payment-settlement-local-context.md",
  evidence:
    "crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json",
  review:
    "crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source-review.json",
};

const wrapper = read(paths.wrapper);
const owner = read(paths.owner);
const doc = read(paths.doc);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, value, expected, label) => {
  const count = content.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(content, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = content.indexOf("{", match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < content.length; index += 1) {
    if (content[index] === "{") depth += 1;
    if (content[index] === "}") {
      depth -= 1;
      if (depth === 0) return content.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return "";
}

const operation = functionBody(wrapper, "settle_checkout_payment");
const facts = functionBody(wrapper, "order_checkout_port_error_facts");
const mapper = functionBody(wrapper, "map_checkout_order_payment_settlement_local_port_error");

for (const marker of [
  "let diagnostic_context = context.clone();",
  "self.inner.settle_checkout_payment(context, request).await",
  "map_checkout_order_payment_settlement_local_port_error(&diagnostic_context, error)",
]) requireText(operation, marker, `${paths.wrapper}: preserved delegation`);

for (const [code, localOperation] of [
  ["order.checkout_payment_request_invalid", "validate_request"],
  ["order.checkout_payment_identity_missing", "require_durable_checkout_identity"],
  ["order.checkout_payment_identity_conflict", "validate_durable_checkout_identity"],
  ["order.checkout_payment_state_conflict", "validate_payment_settlement_lifecycle"],
  ["order.checkout_payment_reference_conflict", "validate_settled_payment_identity"],
]) {
  requireText(wrapper, `"${code}"`, `${paths.wrapper}: stable code ${code}`);
  requireText(wrapper, `"${localOperation}"`, `${paths.wrapper}: local operation ${localOperation}`);
  requireText(owner, `"${code}"`, `${paths.owner}: owner code ${code}`);
}

for (const [variant, label] of [
  ["Validation", "validation"],
  ["NotFound", "not_found"],
  ["Conflict", "conflict"],
  ["Forbidden", "forbidden"],
  ["Unavailable", "unavailable"],
  ["Timeout", "timeout"],
  ["InvariantViolation", "invariant_violation"],
]) requireText(facts, `PortErrorKind::${variant} => "${label}"`, `${paths.wrapper}: ${variant}`);
for (const marker of [
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
]) requireText(facts, marker, `${paths.wrapper}: message shape`);

for (const marker of [
  "checkout_order_payment_settlement_local_operation(error.code.as_str())",
  "let error_facts = order_checkout_port_error_facts(&error);",
  "tracing::error!(",
  "tracing::warn!(",
  "internal_code = %error.code",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "retryable = error.retryable",
  "boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY",
  "\n    error\n}",
]) requireText(mapper, marker, `${paths.wrapper}: bounded local mapper`);
requireCount(mapper, "error_message_present = error_facts.message_present", 2, "two message-presence fields");
requireCount(mapper, "error_message_length = error_facts.message_length", 2, "two message-length fields");
requireCount(mapper, "error_kind = error_facts.error_kind", 2, "two static-kind fields");
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "internal_message",
  "error_kind = ?error.kind",
  "error_kind = %error.kind",
  "error.to_string()",
  "match (error.code.as_str(), error.message.as_str())",
]) forbidText(mapper, forbidden, `${paths.wrapper}: complete PortError payload`);
for (const constructor of [
  "PortError::validation(",
  "PortError::conflict(",
  "PortError::new(",
  "PortError::unavailable(",
  "PortError::invariant_violation(",
]) forbidText(mapper, constructor, `${paths.wrapper}: replacement envelope`);

for (const openMarker of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_payment_owner_error<E: std::fmt::Debug>",
  "order_state = ?order_state",
  "error = ?error",
]) requireText(owner, openMarker, `${paths.owner}: retained open owner gap`);
for (const marker of [
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  ".read_by_operation(",
  ".adopt_legacy(",
  ".mark_paid(",
  "OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered",
]) requireText(owner, marker, `${paths.owner}: preserved settlement behavior`);

if (evidence.status !== "order_checkout_payment_settlement_wrapper_diagnostic_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  local_mapper_event_branch_count: 2,
  complete_port_error_logged_by_local_mapper: false,
  port_error_message_text_logged_by_local_mapper: false,
  static_port_error_kind_logged_by_local_mapper: true,
  port_error_message_shape_logged_by_local_mapper: true,
  same_port_error_returned: true,
  local_mapper_payload_diagnostic_cleanup_closed: true,
  shared_admission_context_payload_diagnostic_cleanup_closed: false,
  canonical_owner_payload_diagnostic_cleanup_closed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
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
  "settlement_replay_proven",
  "concurrent_settlement_proven",
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
  "order_checkout_payment_settlement_wrapper_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.review}: unexpected status ${review.status}`);
for (const [key, expected] of Object.entries({
  complete_port_error_logging_removed_from_local_mapper: true,
  port_error_message_text_logging_removed_from_local_mapper: true,
  static_port_error_kind_logged_by_local_mapper: true,
  port_error_message_shape_logged_by_local_mapper: true,
  local_mapper_payload_diagnostic_cleanup_closed: true,
  shared_admission_context_payload_diagnostic_cleanup_remains_open: true,
  canonical_owner_payload_diagnostic_cleanup_remains_open: true,
  all_public_port_errors_preserved: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  "Status: **wrapper source-closed / owner open / unvalidated**",
  "They do not record the complete `PortError`",
  "shared checkout admission/context events still retain complete `PortError`",
  "canonical settlement owner still retains UUID parser errors",
  "The broad ecommerce correlation-safe mapper cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: source status`);

if (failures.length > 0) {
  console.error("Order payment settlement local diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order payment settlement local mapper retains only static PortError kind and message shape while preserving stable routing and the same public error; owner and shared admission payload cleanup remain open",
);
