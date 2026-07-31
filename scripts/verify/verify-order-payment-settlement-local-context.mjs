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
const portFacts = functionBody(wrapper, "order_checkout_port_error_facts");
const localMapper = functionBody(wrapper, "map_checkout_order_payment_settlement_local_port_error");
const ownerFacts = functionBody(owner, "order_payment_settlement_order_error_facts");
const ownerMapper = functionBody(owner, "order_error_to_port_error");
const parseRejection = functionBody(owner, "log_context_parse_rejection");
const lifecycle = functionBody(owner, "log_payment_settlement_lifecycle_conflict");

for (const marker of [
  "let diagnostic_context = context.clone();",
  "self.inner.settle_checkout_payment(context, request).await",
  "map_checkout_order_payment_settlement_local_port_error(&diagnostic_context, error)",
]) requireText(operation, marker, `${paths.wrapper}: preserved delegation`);

for (const [variant, label] of [
  ["Validation", "validation"],
  ["NotFound", "not_found"],
  ["Conflict", "conflict"],
  ["Forbidden", "forbidden"],
  ["Unavailable", "unavailable"],
  ["Timeout", "timeout"],
  ["InvariantViolation", "invariant_violation"],
]) requireText(portFacts, `PortErrorKind::${variant} => "${label}"`, `${paths.wrapper}: ${variant}`);
for (const marker of [
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "\n    error\n}",
]) requireText(localMapper, marker, `${paths.wrapper}: bounded local mapper`);
requireCount(localMapper, "error_message_present = error_facts.message_present", 2, "two local mapper branches");
for (const forbidden of [
  "error = ?error",
  "internal_message",
  "error_kind = ?error.kind",
]) forbidText(localMapper, forbidden, `${paths.wrapper}: complete PortError payload`);

for (const [variant, label] of [
  ["Database(_)", "database"],
  ["OrderNotFound(order_id)", "order_not_found"],
  ["Validation(cause)", "validation"],
  ["InvalidTransition { from, to }", "invalid_transition"],
  ["OrderReturnNotFound(return_id)", "order_return_not_found"],
  ["OrderChangeNotFound(change_id)", "order_change_not_found"],
  ["Core(_)", "core"],
]) requireText(ownerFacts, `OrderError::${variant}`, `${paths.owner}: ${label}`);
for (const marker of [
  "text_field_count:",
  "text_total_length:",
  "uuid_field_count:",
  "uuid_non_nil_count:",
  "opaque_payload_present:",
  "let error_facts = order_payment_settlement_order_error_facts(&error);",
]) requireText(`${ownerFacts}\n${ownerMapper}`, marker, `${paths.owner}: owner shape`);

requireText(parseRejection, "parse_failed = true", `${paths.owner}: static parse failure`);
for (const forbidden of ["error = ?error", "error: &E", "std::fmt::Debug"]) {
  forbidText(parseRejection, forbidden, `${paths.owner}: parse payload`);
}
for (const marker of [
  "let order_state = order_payment_settlement_status_kind(order_state);",
  "order_state,",
]) requireText(lifecycle, marker, `${paths.owner}: static lifecycle status`);
forbidText(lifecycle, "order_state = ?order_state", `${paths.owner}: debug lifecycle status`);

for (const forbidden of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_payment_owner_error<E: std::fmt::Debug>",
  "error = ?error",
  "from = ?from",
  "to = ?to",
  "internal_cause = %",
]) forbidText(owner, forbidden, `${paths.owner}: complete owner payload`);

if (evidence.status !== "order_checkout_payment_settlement_diagnostic_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  local_mapper_payload_diagnostic_cleanup_closed: true,
  canonical_owner_payload_diagnostic_cleanup_closed: true,
  shared_admission_context_payload_diagnostic_cleanup_closed: false,
  uuid_parse_error_payload_logged_by_owner: false,
  complete_order_error_logged_by_owner: false,
  owner_transition_text_logged: false,
  static_order_error_variant_logged: true,
  static_lifecycle_status_logged: true,
  public_code_changed: false,
  public_message_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
if (review.review_findings?.canonical_owner_payload_diagnostic_cleanup_closed !== true) {
  failures.push(`${paths.review}: owner cleanup must be closed`);
}
if (review.review_findings?.shared_admission_context_payload_diagnostic_cleanup_remains_open !== true) {
  failures.push(`${paths.review}: shared admission/context gap must remain open`);
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "compile_proven",
]) {
  if (evidence.validation?.[key] !== false) failures.push(`${paths.evidence}: ${key} must remain false`);
}

for (const marker of [
  "Status: **wrapper and owner source-closed / shared admission open / unvalidated**",
  "All seven `OrderError` variants are classified by a closed static label.",
  "Shared checkout admission/context events still retain complete `PortError`",
  "The broad ecommerce correlation-safe mapper cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: source status`);

if (failures.length > 0) {
  console.error("Order payment settlement local diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order payment settlement wrapper and canonical owner use bounded diagnostic shape while preserving settlement behavior and leaving shared admission/context cleanup open",
);
