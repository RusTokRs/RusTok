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

for (const marker of [
  'const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";',
  'const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";',
  'const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";',
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  ".read_by_operation(",
  ".adopt_legacy(",
  ".mark_paid(",
  "OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered",
  "state @ (OrderStatusKind::Pending",
  "settled.payment_id.as_deref() == Some(request.payment_reference.as_str())",
  "settled.payment_method.as_deref() == Some(request.payment_method.as_str())",
]) requireText(owner, marker, `${paths.owner}: preserved owner behavior`);

for (const marker of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_payment_owner_error<E: std::fmt::Debug>",
  "error = ?error",
  "order_state = ?order_state",
  "from = ?from",
  "to = ?to",
]) requireText(owner, marker, `${paths.owner}: retained open payload site`);

for (const [variant, operation] of [
  ["OrderError::Database(error)", "owner_storage"],
  ["OrderError::OrderNotFound(order_id)", "load_order"],
  ["OrderError::Validation(cause)", "validate_owner_request"],
  ["OrderError::InvalidTransition { from, to }", "apply_payment_settlement_state"],
  ["OrderError::OrderReturnNotFound(return_id)", "load_related_order_resource"],
  ["OrderError::OrderChangeNotFound(change_id)", "load_related_order_resource"],
  ["OrderError::Core(error)", "owner_invariant"],
]) {
  requireText(owner, variant, `${paths.owner}: mapper variant ${variant}`);
  requireText(owner, `"${operation}"`, `${paths.owner}: mapper operation ${operation}`);
}

for (const [code, message] of [
  ["order.checkout_payment_request_invalid", "checkout payment settlement request is invalid"],
  ["order.checkout_payment_identity_missing", "checkout requires manual reconciliation"],
  ["order.checkout_payment_identity_conflict", "checkout order identity conflicts with the payment settlement request"],
  ["order.checkout_payment_state_conflict", "checkout order lifecycle does not allow payment settlement"],
  ["order.checkout_payment_reference_conflict", "checkout order is settled by another payment identity"],
  ["order.database_unavailable", "order storage is temporarily unavailable"],
  ["order.order_not_found", "order was not found"],
  ["order.checkout_payment_validation", "checkout order payment settlement request is invalid"],
  ["order.related_resource_not_found", "related order resource was not found"],
  ["order.invariant_violation", "order payment settlement failed an internal invariant"],
]) {
  requireText(owner, `"${code}"`, `${paths.owner}: public code ${code}`);
  requireText(owner, `"${message}"`, `${paths.owner}: public message ${message}`);
}

for (const forbidden of [
  "tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "requested_payment_reference = %request.payment_reference",
  "requested_payment_method = %request.payment_method",
  "settled_payment_reference = ?settled.payment_id",
  "settled_payment_method = ?settled.payment_method",
]) forbidText(owner, forbidden, `${paths.owner}: raw identity value`);

if (evidence.status !== "order_checkout_payment_settlement_wrapper_diagnostic_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  canonical_owner_safe_context_shape: true,
  canonical_owner_safe_request_shape: true,
  canonical_owner_safe_identity_shape: true,
  canonical_owner_safe_payment_identity_shape: true,
  canonical_owner_payload_diagnostic_cleanup_closed: false,
  order_error_mapping_changed: false,
  public_code_changed: false,
  public_message_changed: false,
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
if (review.review_findings?.canonical_owner_payload_diagnostic_cleanup_remains_open !== true) {
  failures.push(`${paths.review}: canonical owner payload cleanup must remain open`);
}
if (review.review_findings?.all_public_port_errors_preserved !== true) {
  failures.push(`${paths.review}: public PortError envelopes must remain preserved`);
}
if (review.review_findings?.runtime_evidence_claimed !== false) {
  failures.push(`${paths.review}: runtime evidence must remain unclaimed`);
}

for (const marker of [
  "Status: **wrapper source-closed / owner open / unvalidated**",
  "canonical settlement owner still retains UUID parser errors",
  "The broad ecommerce correlation-safe mapper cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: owner gap disclosure`);

if (failures.length > 0) {
  console.error("Order payment settlement owner diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order payment settlement owner behavior and public envelopes remain fixed while its parser, OrderError, and lifecycle payload cleanup remains an explicit open source slice",
);
