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
  wrapper: "crates/rustok-order/src/checkout_compensation_local_context.rs",
  shared: "crates/rustok-order/src/checkout_owner_context.rs",
  owner: "crates/rustok-order/src/checkout_compensation.rs",
  lib: "crates/rustok-order/src/lib.rs",
  doc: "crates/rustok-order/docs/checkout-compensation-local-context.md",
  ownerDoc: "crates/rustok-order/docs/checkout-owner-context.md",
  plan: "crates/rustok-order/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  evidence:
    "crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json",
  review:
    "crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source-review.json",
  ownerGuard: "scripts/verify/verify-order-checkout-compensation-error-context.mjs",
};

const wrapper = read(paths.wrapper);
const shared = read(paths.shared);
const owner = read(paths.owner);
const lib = read(paths.lib);
const doc = read(paths.doc);
const ownerDoc = read(paths.ownerDoc);
const plan = read(paths.plan);
const commercePlan = read(paths.commercePlan);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const ownerGuard = read(paths.ownerGuard);

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
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < content.length; index += 1) {
    if (content[index] === "{") depth += 1;
    if (content[index] === "}") {
      depth -= 1;
      if (depth === 0) return content.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

for (const marker of [
  "mod checkout_compensation_local_context;",
  '#[path = "checkout_owner_context.rs"]',
  "mod checkout_owner_context_impl;",
  "pub mod checkout_owner_context {",
  "pub use checkout_compensation_local_context::{",
]) requireText(lib, marker, `${paths.lib}: public compensation facade`);

const wrapperOperation = functionBody(wrapper, "compensate_checkout_order");
for (const marker of [
  "let diagnostic_context = context.clone();",
  "let diagnostic_facts = order_compensation_request_facts(&request);",
  "self.inner.compensate_checkout_order(context, request).await",
  "map_checkout_order_compensation_local_port_error(",
]) requireText(wrapperOperation, marker, `${paths.wrapper}: unchanged delegation`);

const wrapperOrder = [
  wrapperOperation.indexOf("let diagnostic_context = context.clone();"),
  wrapperOperation.indexOf("order_compensation_request_facts(&request)"),
  wrapperOperation.indexOf("self.inner.compensate_checkout_order(context, request).await"),
  wrapperOperation.indexOf("map_checkout_order_compensation_local_port_error("),
];
if (
  !wrapperOrder.every(
    (value, index) => value >= 0 && (index === 0 || wrapperOrder[index - 1] < value),
  )
) {
  failures.push(
    `${paths.wrapper}: must retain context/request shape before delegation and map only returned errors`,
  );
}

for (const marker of [
  "struct OrderCompensationContextFacts",
  "struct OrderCompensationRequestFacts",
  "struct OrderCompensationPortErrorFacts",
  "tenant_id_length: context.tenant_id.chars().count()",
  "actor_id_length: context.actor.id.chars().count()",
  "claim_count: context.claims.len()",
  "role_count: context.roles.len()",
  "channel_present: context.channel.is_some()",
  "locale_length: context.locale.chars().count()",
  "causation_id_present: context.causation_id.is_some()",
  "traceparent_present: context.traceparent.is_some()",
  "idempotency_key_present: context.idempotency_key.is_some()",
  "checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil()",
  "cart_id_non_nil: !request.cart_id.is_nil()",
  "expected_order_id_present: request.expected_order_id.is_some()",
  "reason_length: request.reason.as_ref().map(|value| value.chars().count())",
]) requireText(wrapper, marker, `${paths.wrapper}: safe wrapper shape`);

const portErrorFacts = functionBody(wrapper, "order_compensation_port_error_facts");
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
    portErrorFacts,
    `PortErrorKind::${variant} => "${label}"`,
    `${paths.wrapper}: static PortError kind ${variant}`,
  );
}
for (const marker of [
  "message_present: !error.message.trim().is_empty()",
  "message_length: error.message.chars().count()",
]) requireText(portErrorFacts, marker, `${paths.wrapper}: PortError message shape`);

for (const [code, operation] of [
  ["order.checkout_compensation_identity_invalid", "validate_request"],
  ["order.checkout_compensation_identity_conflict", "validate_durable_checkout_identity"],
  ["order.checkout_compensation_state_conflict", "apply_compensation_state"],
  ["order.checkout_compensation_manual_reconciliation", "require_manual_reconciliation"],
]) {
  requireText(wrapper, `"${code}"`, `${paths.wrapper}: code ${code}`);
  requireText(wrapper, `"${operation}"`, `${paths.wrapper}: local operation ${operation}`);
}

const wrapperMapper = functionBody(
  wrapper,
  "map_checkout_order_compensation_local_port_error",
);
for (const marker of [
  "order_compensation_local_operation(error.code.as_str())",
  '"validate_durable_checkout_identity" | "require_manual_reconciliation"',
  "let context_facts = order_compensation_context_facts(context);",
  "let error_facts = order_compensation_port_error_facts(&error);",
  "tracing::error!(",
  "tracing::warn!(",
  "internal_code = %error.code",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "retryable = error.retryable",
  "boundary = ORDER_COMPENSATION_BOUNDARY",
  "\n    error\n}",
]) requireText(wrapperMapper, marker, `${paths.wrapper}: bounded wrapper mapper`);
requireCount(
  wrapperMapper,
  "error_message_present = error_facts.message_present",
  2,
  "two wrapper message presence fields",
);
requireCount(
  wrapperMapper,
  "error_message_length = error_facts.message_length",
  2,
  "two wrapper message length fields",
);
requireCount(
  wrapperMapper,
  "error_kind = error_facts.error_kind",
  2,
  "two wrapper kind fields",
);
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "internal_message",
  "error_kind = ?error.kind",
  "error_kind = %error.kind",
  "error.to_string()",
  "match (error.code.as_str(), error.message.as_str())",
]) forbidText(wrapperMapper, forbidden, `${paths.wrapper}: complete PortError payload`);

for (const marker of [
  "struct OrderCheckoutContextFacts",
  "fn order_checkout_context_facts(",
  "checkout_order_payment_settlement_local_operation(error.code.as_str())",
  "expected_checkout_operation_id_present",
  "expected_checkout_operation_id_non_nil",
  "causation_matches = false",
]) requireText(shared, marker, `${paths.shared}: shared admission context`);
forbidText(
  shared,
  "match (error.code.as_str(), error.message.as_str())",
  `${paths.shared}: message-pair classifier`,
);

for (const marker of [
  'const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";',
  "struct OrderCompensationContextFacts",
  "struct OrderCompensationOrderErrorFacts",
  "fn order_compensation_order_error_facts(",
  "fn order_status_kind_label(",
  "fn log_invalid_compensation_request(",
  "fn log_compensation_transition_conflict(",
  "tenant_matches",
  "checkout_operation_matches",
  "source_cart_matches",
  "expected_order_matches",
  "request_checkout_operation_id_non_nil",
  "identity_order_id_non_nil",
  "parse_failed = true",
  "reconciliation_reason_present,",
  "reconciliation_reason_length,",
  "order_error_variant = error_facts.error_variant",
  "order_error_opaque_payload_present = error_facts.opaque_payload_present",
  "fn log_order_owner_warning(",
  "fn log_order_owner_error(",
]) requireText(owner, marker, `${paths.owner}: bounded owner diagnostics`);

for (const [content, label] of [
  [wrapper, paths.wrapper],
  [shared, paths.shared],
  [owner, paths.owner],
]) {
  for (const forbidden of [
    "tenant_id = %context.tenant_id",
    "actor = ?context.actor",
    "channel = ?context.channel",
    "locale = %context.locale",
    "causation_id = ?context.causation_id",
    "traceparent = ?context.traceparent",
    "idempotency_key = ?context.idempotency_key",
    "checkout_operation_id = %",
    "cart_id = %",
    "order_id = %",
    "resource_id = %",
    "request_checkout_operation_id = %",
    "identity_tenant_id = %",
    "identity_order_id = %",
    "reason = %reason",
  ]) forbidText(content, forbidden, `${label}: raw diagnostic value`);
}
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "internal_cause = ?internal_cause",
  "from = ?from",
  "to = ?to",
  "internal_reason = reason",
  "current_state = ?current_state",
]) forbidText(owner, forbidden, `${paths.owner}: raw error payload`);

for (const marker of [
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  "let tenant_id = parse_tenant_id(&context, COMPENSATE_OPERATION)?;",
  "let actor_id = parse_actor_id(&context, COMPENSATE_OPERATION)?;",
  "request.checkout_operation_id",
  ".read_by_operation(",
  ".adopt_legacy(",
  "return if request.expected_order_id.is_none()",
  "validate_identity(&context, tenant_id, &request, &identity)?;",
  ".get_order(tenant_id, identity.order_id)",
  ".cancel_order(tenant_id, actor_id, order.id, reason)",
  "if current.status_kind() == OrderStatusKind::Cancelled",
  "state @ (OrderStatusKind::Paid",
]) requireText(owner, marker, `${paths.owner}: preserved compensation behavior`);

for (const message of [
  "checkout compensation request is invalid",
  "checkout order identity conflicts with the compensation request",
  "checkout order changed while compensation was being applied",
  "checkout requires manual reconciliation",
  "order storage is temporarily unavailable",
  "order was not found",
  "checkout order compensation request is invalid",
  "checkout order lifecycle conflicts with compensation",
  "related order resource was not found",
  "order compensation failed an internal invariant",
]) requireText(owner, `"${message}"`, `${paths.owner}: public message ${message}`);

if (evidence.status !== "order_checkout_compensation_diagnostic_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "order_checkout_compensation_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.review}: unexpected status ${review.status}`);

for (const [key, expected] of Object.entries({
  wrapper_event_branch_count: 2,
  context_parse_site_count: 2,
  order_error_variant_count: 7,
  transition_conflict_site_count: 1,
  manual_reconciliation_call_site_count: 3,
  stable_code_only_local_classification: true,
  human_message_control_flow: false,
  complete_port_error_logged_by_wrapper: false,
  port_error_message_text_logged_by_wrapper: false,
  static_port_error_kind_logged_by_wrapper: true,
  port_error_message_shape_logged_by_wrapper: true,
  complete_order_error_logged_by_owner: false,
  order_error_variant_shape_logged_by_owner: true,
  context_parse_failure_flag_logged_by_owner: true,
  transition_shape_logged_by_owner: true,
  manual_reconciliation_reason_shape_logged_by_owner: true,
  delegated_port_error_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  write_admission_order_changed: false,
  identity_read_or_legacy_adoption_changed: false,
  cancellation_or_replay_policy_changed: false,
  safe_context_shape_logged: true,
  safe_request_shape_logged: true,
  checkout_order_compensation_payload_diagnostic_cleanup_closed: true,
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
  "compensation_replay_proven",
  "restart_proven",
  "remote_port_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  public_facade_preserved: true,
  compensation_wrapper_delegation_preserved: true,
  shared_admission_order_preserved: true,
  stable_code_only_wrapper_classifier_present: true,
  complete_port_error_logging_removed_from_wrapper: true,
  port_error_message_text_logging_removed_from_wrapper: true,
  complete_order_error_logging_removed_from_owner: true,
  order_error_payload_shape_only: true,
  context_parse_error_text_logging_removed: true,
  transition_text_logging_removed: true,
  manual_reconciliation_reason_text_logging_removed: true,
  same_port_error_returned: true,
  identity_read_and_legacy_adoption_preserved: true,
  cancellation_race_and_cancelled_adoption_preserved: true,
  typed_lifecycle_reconciliation_preserved: true,
  public_error_mapping_preserved: true,
  checkout_order_compensation_payload_diagnostic_cleanup_closed: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "They do not record the complete `PortError`",
  "The seven `OrderError` variants now retain only:",
  "Parser error text is not logged.",
  "source-closed / unvalidated",
  "The broad ecommerce correlation-safe mapper item remains open",
]) requireText(doc, marker, `${paths.doc}: compensation documentation`);
for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are",
  "Payment-settlement owner-local request/identity/lifecycle diagnostics remain a",
]) requireText(ownerDoc, marker, `${paths.ownerDoc}: owner-context documentation`);
requireText(
  plan,
  "The currently identified checkout compensation wrapper and owner",
  `${paths.plan}: compensation diagnostic status`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup for order, payment execution/compensation,",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);
requireText(
  ownerGuard,
  "Order checkout compensation owner diagnostic-safety verification failed:",
  `${paths.ownerGuard}: focused owner guard`,
);

if (failures.length > 0) {
  console.error("Order compensation diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order checkout compensation wrapper and owner retain only bounded PortError, OrderError, parse, transition, and reconciliation shape while preserving public envelopes and owner behavior; runtime evidence remains open",
);
