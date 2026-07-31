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
  source: "crates/rustok-order/src/checkout_compensation.rs",
  evidence:
    "crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json",
  review:
    "crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source-review.json",
  doc: "crates/rustok-order/docs/checkout-compensation-local-context.md",
  plan: "crates/rustok-order/docs/implementation-plan.md",
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const plan = read(paths.plan);

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

const errorFacts = functionBody(source, "order_compensation_order_error_facts");
const transition = functionBody(source, "log_compensation_transition_conflict");
const parseRejection = functionBody(source, "log_context_parse_rejection");
const reconciliation = functionBody(source, "manual_reconciliation");
const warning = functionBody(source, "log_order_owner_warning");
const technical = functionBody(source, "log_order_owner_error");
const mapper = functionBody(source, "order_error_to_port_error");

for (const marker of [
  'const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";',
  'const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";',
  'const COMPENSATE_OPERATION: &str = "compensate_checkout_order";',
  "struct OrderCompensationContextFacts",
  "struct OrderCompensationOrderErrorFacts",
  "fn order_status_kind_label(",
  'OrderStatusKind::Pending => "pending"',
  'OrderStatusKind::Confirmed => "confirmed"',
  'OrderStatusKind::Paid => "paid"',
  'OrderStatusKind::Shipped => "shipped"',
  'OrderStatusKind::Delivered => "delivered"',
  'OrderStatusKind::Cancelled => "cancelled"',
  'OrderStatusKind::Unknown => "unknown"',
]) requireText(source, marker, `${paths.source}: bounded fact model`);

for (const marker of [
  'OrderError::Database(_) => ("database", 0, 0, 0, 0, true)',
  '"order_not_found"',
  '"validation"',
  '"invalid_transition"',
  '"order_return_not_found"',
  '"order_change_not_found"',
  'OrderError::Core(_) => ("core", 0, 0, 0, 0, true)',
  "text_field_count",
  "text_total_length",
  "uuid_field_count",
  "uuid_non_nil_count",
  "opaque_payload_present",
]) requireText(errorFacts, marker, `${paths.source}: OrderError shape`);

for (const forbidden of [
  "format!(",
  ".to_string()",
  "debug_error",
  "error_text",
  "uuid_value",
]) forbidText(errorFacts, forbidden, `${paths.source}: OrderError payload values`);

for (const marker of [
  "let current_state = order_status_kind_label(current_state);",
  "transition_from_present = !from.trim().is_empty()",
  "transition_from_length = from.chars().count()",
  "transition_to_present = !to.trim().is_empty()",
  "transition_to_length = to.chars().count()",
  "current_state,",
  "order_id_non_nil = !order_id.is_nil()",
]) requireText(transition, marker, `${paths.source}: transition shape`);
for (const forbidden of [
  "current_state = ?current_state",
  "\n        from,\n",
  "\n        to,\n",
  "from = %",
  "to = %",
]) forbidText(transition, forbidden, `${paths.source}: transition text`);

for (const marker of [
  "parse_failed = true",
  "field,",
  "value_length,",
  "correlation_id = %context.correlation_id",
  "boundary = ORDER_COMPENSATION_BOUNDARY",
]) requireText(parseRejection, marker, `${paths.source}: parse failure shape`);
for (const forbidden of [
  "error = ?error",
  "error = %error",
  "error.to_string()",
  "std::fmt::Debug",
]) forbidText(parseRejection, forbidden, `${paths.source}: parser payload`);

for (const marker of [
  "let order_state = order_status_kind_label(order_state);",
  "reconciliation_reason_present = !reason.trim().is_empty()",
  "reconciliation_reason_length = reason.chars().count()",
  "order_id_present = order_id.is_some()",
  "order_id_non_nil = ?order_id.map(|value| !value.is_nil())",
  "order_state,",
  "reconciliation_reason_present,",
  "reconciliation_reason_length,",
  '"order.checkout_compensation_manual_reconciliation"',
  '"checkout requires manual reconciliation"',
]) requireText(reconciliation, marker, `${paths.source}: reconciliation shape`);
for (const forbidden of [
  "internal_reason = reason",
  "\n        reason,\n",
  "order_state = ?order_state",
]) forbidText(reconciliation, forbidden, `${paths.source}: reconciliation payload`);

for (const body of [warning, technical]) {
  for (const marker of [
    "order_error_variant = error_facts.error_variant",
    "order_error_text_field_count = error_facts.text_field_count",
    "order_error_text_total_length = error_facts.text_total_length",
    "order_error_uuid_field_count = error_facts.uuid_field_count",
    "order_error_uuid_non_nil_count = error_facts.uuid_non_nil_count",
    "order_error_opaque_payload_present = error_facts.opaque_payload_present",
    "correlation_id = %context.correlation_id",
    "boundary = ORDER_COMPENSATION_BOUNDARY",
  ]) requireText(body, marker, `${paths.source}: bounded mapper diagnostics`);
  for (const forbidden of [
    "error = ?error",
    "error = %error",
    "internal_cause",
    "from = ?from",
    "to = ?to",
    "resource_id = %",
  ]) forbidText(body, forbidden, `${paths.source}: complete mapper payload`);
}
requireText(warning, "resource = ?resource", `${paths.source}: static resource kind`);

for (const marker of [
  "let error_facts = order_compensation_order_error_facts(&error);",
  "OrderError::Database(_)",
  "OrderError::OrderNotFound(_)",
  "OrderError::Validation(_)",
  "OrderError::InvalidTransition { .. }",
  "OrderError::OrderReturnNotFound(_)",
  "OrderError::OrderChangeNotFound(_)",
  "OrderError::Core(_)",
  '"owner_storage"',
  '"load_order"',
  '"validate_owner_request"',
  '"apply_compensation_state"',
  '"load_related_order_resource"',
  '"owner_invariant"',
]) requireText(mapper, marker, `${paths.source}: preserved mapper variants`);

requireCount(source, "log_context_parse_rejection(", 3, "two parse sites plus helper");
requireCount(source, "log_order_owner_warning(", 6, "five warning sites plus helper");
requireCount(source, "log_order_owner_error(", 3, "two technical sites plus helper");
requireCount(source, "manual_reconciliation(", 4, "three routes plus helper");
requireCount(source, "log_compensation_transition_conflict(", 2, "one site plus helper");

for (const forbidden of [
  "fn log_context_parse_rejection<E: std::fmt::Debug>",
  "fn log_order_owner_error<E: std::fmt::Debug>",
  "error = ?error",
  "error = %error",
  "internal_cause = ?internal_cause",
  "from = ?from",
  "to = ?to",
  "internal_reason = reason",
  "current_state = ?current_state",
]) forbidText(source, forbidden, `${paths.source}: raw owner payload`);

for (const marker of [
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  "let tenant_id = parse_tenant_id(&context, COMPENSATE_OPERATION)?;",
  "let actor_id = parse_actor_id(&context, COMPENSATE_OPERATION)?;",
  "require_operation_context(",
  ".read_by_operation(",
  ".adopt_legacy(",
  "return if request.expected_order_id.is_none()",
  "validate_identity(&context, tenant_id, &request, &identity)?;",
  ".get_order(tenant_id, identity.order_id)",
  ".cancel_order(tenant_id, actor_id, order.id, reason)",
  "if current.status_kind() == OrderStatusKind::Cancelled",
  "state @ (OrderStatusKind::Paid",
]) requireText(source, marker, `${paths.source}: preserved compensation flow`);

for (const [code, message] of [
  ["order.checkout_compensation_identity_invalid", "checkout compensation request is invalid"],
  ["order.checkout_compensation_identity_conflict", "checkout order identity conflicts with the compensation request"],
  ["order.checkout_compensation_state_conflict", "checkout order changed while compensation was being applied"],
  ["order.checkout_compensation_manual_reconciliation", "checkout requires manual reconciliation"],
  ["order.database_unavailable", "order storage is temporarily unavailable"],
  ["order.order_not_found", "order was not found"],
  ["order.checkout_compensation_validation", "checkout order compensation request is invalid"],
  ["order.related_resource_not_found", "related order resource was not found"],
  ["order.invariant_violation", "order compensation failed an internal invariant"],
]) {
  requireText(source, `"${code}"`, `${paths.source}: public code ${code}`);
  requireText(source, `"${message}"`, `${paths.source}: public message ${message}`);
}

if (evidence.status !== "order_checkout_compensation_diagnostic_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  wrapper_event_branch_count: 2,
  context_parse_site_count: 2,
  order_error_variant_count: 7,
  transition_conflict_site_count: 1,
  manual_reconciliation_call_site_count: 3,
  complete_order_error_logged_by_owner: false,
  order_error_text_logged_by_owner: false,
  order_error_uuid_value_logged_by_owner: false,
  order_error_opaque_payload_logged_by_owner: false,
  order_error_variant_shape_logged_by_owner: true,
  context_parse_error_text_logged_by_owner: false,
  context_parse_failure_flag_logged_by_owner: true,
  transition_text_logged_by_owner: false,
  transition_shape_logged_by_owner: true,
  manual_reconciliation_reason_text_logged_by_owner: false,
  manual_reconciliation_reason_shape_logged_by_owner: true,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  cancellation_or_replay_policy_changed: false,
  manual_reconciliation_policy_changed: false,
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

if (
  review.status !==
  "order_checkout_compensation_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.review}: unexpected status ${review.status}`);
for (const [key, expected] of Object.entries({
  complete_order_error_logging_removed_from_owner: true,
  order_error_payload_shape_only: true,
  context_parse_error_text_logging_removed: true,
  transition_text_logging_removed: true,
  manual_reconciliation_reason_text_logging_removed: true,
  public_error_mapping_preserved: true,
  warning_error_severity_preserved: true,
  checkout_order_compensation_payload_diagnostic_cleanup_closed: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "The seven `OrderError` variants now retain only:",
  "Parser error text is not logged.",
  "They do not retain raw transition text.",
  "It does not retain internal reason text.",
  "source-closed / unvalidated",
]) requireText(doc, marker, `${paths.doc}: owner payload policy`);
requireText(
  plan,
  "The currently identified checkout compensation wrapper and owner",
  `${paths.plan}: compensation diagnostic status`,
);

if (failures.length > 0) {
  console.error("Order checkout compensation owner diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Order checkout compensation owner diagnostics retain only static OrderError, parse, transition, and reconciliation shape while preserving public envelopes and lifecycle behavior",
);
