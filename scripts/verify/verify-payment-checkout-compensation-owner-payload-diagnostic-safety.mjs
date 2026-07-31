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
  owner: "crates/rustok-payment/src/checkout_compensation.rs",
  ownerEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source.json",
  ownerReview:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source-review.json",
  wrapperEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json",
  wrapperReview:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json",
  doc: "crates/rustok-payment/docs/checkout-compensation-local-context.md",
  paymentPlan: "crates/rustok-payment/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const owner = read(paths.owner);
const ownerEvidence = JSON.parse(read(paths.ownerEvidence));
const ownerReview = JSON.parse(read(paths.ownerReview));
const wrapperEvidence = JSON.parse(read(paths.wrapperEvidence));
const wrapperReview = JSON.parse(read(paths.wrapperReview));
const doc = read(paths.doc);
const paymentPlan = read(paths.paymentPlan);
const commercePlan = read(paths.commercePlan);

const paymentFacts = functionBody(
  owner,
  "checkout_payment_compensation_payment_error_facts",
);
const ownerFailure = functionBody(
  owner,
  "log_checkout_payment_compensation_owner_failure",
);
const paymentFailure = functionBody(
  owner,
  "log_checkout_payment_compensation_payment_error",
);
const staticFailure = functionBody(
  owner,
  "log_checkout_payment_compensation_static_error",
);
const tenantWarning = functionBody(
  owner,
  "log_checkout_payment_compensation_owner_warning",
);
const reconciliation = functionBody(owner, "manual_reconciliation");
const mapper = functionBody(owner, "payment_error_to_port_error");

for (const marker of [
  "struct CheckoutPaymentCompensationPaymentErrorFacts {",
  "enum CheckoutPaymentCompensationOwnerFailureKind {",
  "ProviderRequestEncoding,",
  "ProviderResultEncoding,",
  "ProviderResultDecoding,",
  'Self::ProviderRequestEncoding => "provider_request_encoding"',
  'Self::ProviderResultEncoding => "provider_result_encoding"',
  'Self::ProviderResultDecoding => "provider_result_decoding"',
]) {
  requireText(owner, marker, `${paths.owner}: failure fact model`);
}

for (const marker of [
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
]) {
  requireText(paymentFacts, marker, `${paths.owner}: PaymentError shape ${marker}`);
}

for (const marker of [
  "failure_kind: &'static str",
  "payment_error_facts: Option<CheckoutPaymentCompensationPaymentErrorFacts>",
  "failure_kind,",
  "payment_error_variant = ?payment_error_variant",
  "payment_error_text_field_count = ?payment_error_text_field_count",
  "payment_error_text_total_length = ?payment_error_text_total_length",
  "payment_error_uuid_field_count = ?payment_error_uuid_field_count",
  "payment_error_uuid_non_nil_count = ?payment_error_uuid_non_nil_count",
  "payment_error_opaque_payload_present = ?payment_error_opaque_payload_present",
  "operation_id_present,",
  "operation_id_non_nil = ?operation_id_non_nil",
  "correlation_id = %context.correlation_id",
  "boundary = PAYMENT_COMPENSATION_BOUNDARY",
]) {
  requireText(ownerFailure, marker, `${paths.owner}: bounded owner failure`);
}

for (const forbidden of [
  "error = ?error",
  "error = %error",
  "error = ?checkpoint_error",
  "error = %checkpoint_error",
  "error.to_string()",
  "checkpoint_error.to_string()",
  "serde_error",
]) {
  forbidText(ownerFailure, forbidden, `${paths.owner}: complete owner failure payload`);
}

for (const marker of [
  '"payment_error"',
  "Some(checkout_payment_compensation_payment_error_facts(error))",
]) {
  requireText(paymentFailure, marker, `${paths.owner}: PaymentError adapter`);
}
for (const marker of ["failure_kind.label()", "None,"]) {
  requireText(staticFailure, marker, `${paths.owner}: static codec adapter`);
}

for (const marker of [
  "serde_json::to_value(&provider_request).map_err(|_|",
  "CheckoutPaymentCompensationOwnerFailureKind::ProviderRequestEncoding",
  "serde_json::to_value(&provider_result).map_err(|_|",
  "CheckoutPaymentCompensationOwnerFailureKind::ProviderResultEncoding",
  "serde_json::from_value(value).map(Some).map_err(|_|",
  "CheckoutPaymentCompensationOwnerFailureKind::ProviderResultDecoding",
]) {
  requireText(owner, marker, `${paths.owner}: static codec failure site`);
}

for (const marker of [
  "tenant_id_parse_failed = true",
  "tenant_id_length = context_facts.tenant_id_length",
  "correlation_id = %context.correlation_id",
  'code = "payment.tenant_id_invalid"',
]) {
  requireText(tenantWarning, marker, `${paths.owner}: tenant parse facts`);
}
for (const forbidden of ["error = ?error", "error = %error", "error.to_string()"])
  forbidText(tenantWarning, forbidden, `${paths.owner}: tenant parse payload`);

for (const marker of [
  "reconciliation_reason_present = !internal_message.trim().is_empty()",
  "reconciliation_reason_length = internal_message.chars().count()",
  "reconciliation_reason_present,",
  "reconciliation_reason_length,",
  'code = "payment.checkout_compensation_manual_reconciliation"',
  "PortErrorKind::Conflict",
  '"payment checkout compensation requires manual reconciliation"',
  "false,",
]) {
  requireText(reconciliation, marker, `${paths.owner}: reconciliation reason shape`);
}
forbidText(
  reconciliation,
  "\n        internal_message,\n",
  `${paths.owner}: reconciliation reason text`,
);

requireCount(
  owner,
  "log_checkout_payment_compensation_payment_error(",
  6,
  "five PaymentError call sites plus helper",
);
requireCount(
  owner,
  "log_checkout_payment_compensation_static_error(",
  4,
  "three codec call sites plus helper",
);
requireCount(
  owner,
  "manual_reconciliation(",
  18,
  "seventeen reconciliation call sites plus helper",
);
requireCount(owner, "tenant_id_parse_failed = true", 1, "one tenant parse site");

for (const forbidden of [
  "fn log_checkout_payment_compensation_owner_error<",
  "fn log_checkout_payment_compensation_owner_warning<",
  "error = ?error",
  "error = %error",
  "error = ?checkpoint_error",
  "error = %checkpoint_error",
  "\n        internal_message,\n",
]) {
  forbidText(owner, forbidden, `${paths.owner}: generic payload logging`);
}

for (const marker of [
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  "parse_tenant_id(&context, owner_operation)?;",
  "require_operation_context(&context, owner_operation, request.checkout_operation_id)?;",
  "PaymentCollectionStatusKind::Captured",
  "PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized",
  "PaymentCollectionStatusKind::Unknown",
  'format!("payment_collection:{}:cancel", collection.id)',
  '"operation": "cancel_payment_collection"',
  ".execute_cancel(provider_id.as_str(), provider_request)",
  ".begin(BeginProviderOperation {",
  ".claim_execution(operation.id)",
  ".mark_reconciliation_required(operation.id, code)",
  ".mark_provider_error(operation.id, code)",
  ".mark_provider_succeeded(",
  ".mark_committed(outcome.operation_id)",
  "persisted_cancel_result(context, owner_operation, &operation)",
]) {
  requireText(owner, marker, `${paths.owner}: preserved owner behavior`);
}

for (const marker of [
  "PaymentError::Database(_) => PortError::unavailable(",
  "PaymentError::Validation(_) => PortError::validation(",
  "PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(",
  "PaymentError::InvalidTransition { .. } => PortError::conflict(",
  "PaymentError::ProviderUnavailable { .. } => PortError::unavailable(",
  "PaymentError::ProviderRejected { .. } => PortError::conflict(",
  "PaymentError::ProviderInvalidResponse { .. } => PortError::invariant_violation(",
  "PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(",
  "PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(",
]) {
  requireText(mapper, marker, `${paths.owner}: preserved public mapper`);
}

if (
  ownerEvidence.status !==
  "payment_checkout_compensation_owner_diagnostic_safety_source_unvalidated"
) failures.push(`${paths.ownerEvidence}: unexpected status ${ownerEvidence.status}`);

for (const [key, expected] of Object.entries({
  owner_error_site_count: 8,
  payment_error_site_count: 5,
  codec_error_site_count: 3,
  tenant_parse_site_count: 1,
  manual_reconciliation_call_site_count: 17,
  complete_internal_error_logged: false,
  payment_error_text_logged: false,
  payment_error_uuid_value_logged: false,
  payment_error_opaque_payload_logged: false,
  payment_error_variant_logged: true,
  codec_error_text_logged: false,
  static_codec_failure_kind_logged: true,
  tenant_parse_error_text_logged: false,
  tenant_parse_failure_flag_logged: true,
  manual_reconciliation_reason_text_logged: false,
  manual_reconciliation_reason_shape_logged: true,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  provider_cancel_policy_changed: false,
  provider_journal_policy_changed: false,
  provider_replay_policy_changed: false,
  manual_reconciliation_envelope_changed: false,
  payment_error_mapping_changed: false,
  persistent_owner_diagnostic_cleanup_complete: true,
  checkout_compensation_payload_diagnostic_cleanup_closed: true,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (ownerEvidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.ownerEvidence}: source_contract.${key} must be ${expected}`);
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
  "production_behavior_proven",
]) {
  if (ownerEvidence.validation?.[key] !== false) {
    failures.push(`${paths.ownerEvidence}: validation.${key} must remain false`);
  }
}

if (
  ownerReview.status !==
  "payment_checkout_compensation_owner_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.ownerReview}: unexpected status ${ownerReview.status}`);
for (const [key, expected] of Object.entries({
  complete_internal_error_logging_removed: true,
  payment_error_payload_shape_only: true,
  codec_error_text_logging_removed: true,
  tenant_parse_error_text_logging_removed: true,
  manual_reconciliation_reason_text_logging_removed: true,
  persistent_owner_diagnostic_cleanup_complete: true,
  checkout_compensation_payload_diagnostic_cleanup_closed: true,
  focused_owner_guard_added: true,
  runtime_evidence_claimed: false,
})) {
  if (ownerReview.review_findings?.[key] !== expected) {
    failures.push(`${paths.ownerReview}: review_findings.${key} must be ${expected}`);
  }
}

for (const [companionPath, companion] of [
  [paths.wrapperEvidence, wrapperEvidence],
  [paths.wrapperReview, wrapperReview],
]) {
  const contract = companion.source_contract ?? companion.review_findings;
  if (contract?.persistent_owner_diagnostic_cleanup_complete !== true) {
    failures.push(`${companionPath}: persistent owner diagnostic cleanup must be true`);
  }
  if (contract?.checkout_compensation_payload_diagnostic_cleanup_closed !== true) {
    failures.push(`${companionPath}: compensation payload diagnostics must be source-closed`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Five `PaymentError` event sites retain only:",
  "Three codec event sites retain only static",
  "Manual reconciliation retains only",
  "payload-diagnostic sites at source level",
  "No FBA or FFA status is promoted from source inspection.",
]) {
  requireText(doc, marker, `${paths.doc}: owner payload policy`);
}
requireText(
  paymentPlan,
  "The currently identified checkout compensation payload-diagnostic sites are source-closed",
  `${paths.paymentPlan}: payment source status`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup for order, payment execution/compensation,",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout compensation owner payload diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout compensation owner diagnostics retain only PaymentError shape or static codec/parse/reconciliation facts; execution validation remains open",
);
