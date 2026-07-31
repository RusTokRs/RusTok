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
  owner: "crates/rustok-order/src/checkout_payment_settlement.rs",
  doc: "crates/rustok-order/docs/checkout-payment-settlement-local-context.md",
  evidence:
    "crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json",
  review:
    "crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source-review.json",
};

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

const operation = functionBody(owner, "settle_checkout_payment");
const statusFacts = functionBody(owner, "order_payment_settlement_status_kind");
const errorFacts = functionBody(owner, "order_payment_settlement_order_error_facts");
const parseRejection = functionBody(owner, "log_context_parse_rejection");
const warning = functionBody(owner, "log_order_payment_owner_warning");
const technical = functionBody(owner, "log_order_payment_owner_error");
const mapper = functionBody(owner, "order_error_to_port_error");

const orderedMarkers = [
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  "parse_tenant_id(&context, SETTLE_PAYMENT_OPERATION)?;",
  "parse_actor_id(&context, SETTLE_PAYMENT_OPERATION)?;",
  "require_operation_context(",
  "validate_request(&context, &request)?;",
  ".read_by_operation(",
  ".adopt_legacy(",
  "validate_identity(&context, tenant_id, &request, &identity)?;",
  "let current = self.load_order(&context, tenant_id, &request).await?;",
];
const positions = orderedMarkers.map((marker) => operation.indexOf(marker));
if (!positions.every((value, index) => value >= 0 && (index === 0 || positions[index - 1] < value))) {
  failures.push(`${paths.owner}: settlement admission/identity order changed`);
}
for (const marker of [
  "OrderStatusKind::Confirmed => self",
  ".mark_paid(",
  "request.payment_reference.clone()",
  "request.payment_method.clone()",
  "OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered",
  "state @ (OrderStatusKind::Pending",
  "settled.payment_id.as_deref() == Some(request.payment_reference.as_str())",
  "settled.payment_method.as_deref() == Some(request.payment_method.as_str())",
]) requireText(operation, marker, `${paths.owner}: preserved settlement behavior`);

for (const [variant, label] of [
  ["Pending", "pending"],
  ["Confirmed", "confirmed"],
  ["Paid", "paid"],
  ["Shipped", "shipped"],
  ["Delivered", "delivered"],
  ["Cancelled", "cancelled"],
  ["Unknown", "unknown"],
]) requireText(statusFacts, `OrderStatusKind::${variant} => "${label}"`, `${paths.owner}: status ${variant}`);

for (const [variant, label] of [
  ["Database(_)", "database"],
  ["OrderNotFound(order_id)", "order_not_found"],
  ["Validation(cause)", "validation"],
  ["InvalidTransition { from, to }", "invalid_transition"],
  ["OrderReturnNotFound(return_id)", "order_return_not_found"],
  ["OrderChangeNotFound(change_id)", "order_change_not_found"],
  ["Core(_)", "core"],
]) {
  requireText(errorFacts, `OrderError::${variant}`, `${paths.owner}: facts ${variant}`);
  requireText(errorFacts, `error_variant: "${label}"`, `${paths.owner}: label ${label}`);
}
requireCount(errorFacts, "opaque_payload_present: true", 2, "two opaque owner variants");
for (const marker of [
  "text_field_count:",
  "text_total_length:",
  "uuid_field_count:",
  "uuid_non_nil_count:",
]) requireText(errorFacts, marker, `${paths.owner}: aggregate error shape`);

requireCount(owner, "map_err(|_|", 2, "two static UUID parse paths");
requireText(parseRejection, "parse_failed = true", `${paths.owner}: parse failure flag`);
for (const forbidden of ["error = ?error", "error: &E", "std::fmt::Debug"]) {
  forbidText(parseRejection, forbidden, `${paths.owner}: UUID parser payload`);
}

for (const block of [warning, technical]) {
  for (const marker of [
    "error_variant = error_facts.error_variant",
    "text_field_count = error_facts.text_field_count",
    "text_total_length = error_facts.text_total_length",
    "uuid_field_count = error_facts.uuid_field_count",
    "uuid_non_nil_count = error_facts.uuid_non_nil_count",
    "opaque_payload_present = error_facts.opaque_payload_present",
  ]) requireText(block, marker, `${paths.owner}: bounded owner event`);
  for (const forbidden of ["error = ?error", "from = ?from", "to = ?to", "internal_cause = %"]) {
    forbidText(block, forbidden, `${paths.owner}: complete owner payload`);
  }
}

requireText(mapper, "let error_facts = order_payment_settlement_order_error_facts(&error);", `${paths.owner}: facts before mapper`);
for (const [variant, constructor, code, message] of [
  ["OrderError::Database(_)", "PortError::unavailable(", "order.database_unavailable", "order storage is temporarily unavailable"],
  ["OrderError::OrderNotFound(order_id)", "PortError::not_found(", "order.order_not_found", "order was not found"],
  ["OrderError::Validation(_)", "PortError::validation(", "order.checkout_payment_validation", "checkout order payment settlement request is invalid"],
  ["OrderError::InvalidTransition { .. }", "PortError::conflict(", "order.checkout_payment_state_conflict", "order lifecycle conflicts with payment settlement"],
  ["OrderError::OrderReturnNotFound(return_id)", "PortError::not_found(", "order.related_resource_not_found", "related order resource was not found"],
  ["OrderError::OrderChangeNotFound(change_id)", "PortError::not_found(", "order.related_resource_not_found", "related order resource was not found"],
  ["OrderError::Core(_)", "PortError::invariant_violation(", "order.invariant_violation", "order payment settlement failed an internal invariant"],
]) {
  requireText(mapper, variant, `${paths.owner}: mapper ${variant}`);
  requireText(mapper, constructor, `${paths.owner}: constructor ${constructor}`);
  requireText(mapper, `"${code}"`, `${paths.owner}: code ${code}`);
  requireText(mapper, `"${message}"`, `${paths.owner}: message ${message}`);
}

for (const forbidden of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_payment_owner_error<E: std::fmt::Debug>",
  "order_state = ?order_state",
  "error = ?error",
  "from = ?from",
  "to = ?to",
  "cause = %cause",
]) forbidText(owner, forbidden, `${paths.owner}: unsafe owner diagnostic`);

if (evidence.source_contract?.owner_context_parse_event_count !== 2) {
  failures.push(`${paths.evidence}: owner_context_parse_event_count must be 2`);
}
if (evidence.source_contract?.order_error_variant_count !== 7) {
  failures.push(`${paths.evidence}: order_error_variant_count must be 7`);
}
if (evidence.source_contract?.canonical_owner_payload_diagnostic_cleanup_closed !== true) {
  failures.push(`${paths.evidence}: canonical owner cleanup must be closed`);
}
if (evidence.source_contract?.shared_admission_context_payload_diagnostic_cleanup_closed !== false) {
  failures.push(`${paths.evidence}: shared admission/context cleanup must remain open`);
}
if (review.review_findings?.all_public_port_errors_preserved !== true) {
  failures.push(`${paths.review}: all public PortError envelopes must be preserved`);
}
if (review.review_findings?.runtime_evidence_claimed !== false) {
  failures.push(`${paths.review}: runtime evidence must remain unclaimed`);
}

for (const marker of [
  "All seven `OrderError` variants are classified by a closed static label.",
  "Database and core payloads are not formatted.",
  "Lifecycle rejection uses a closed seven-value",
  "Shared checkout admission/context events still retain complete `PortError`",
]) requireText(doc, marker, `${paths.doc}: owner closure disclosure`);

if (failures.length > 0) {
  console.error("Order payment settlement owner diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order payment settlement canonical owner uses static parse, OrderError, and lifecycle facts while preserving settlement flow and public PortError envelopes",
);
