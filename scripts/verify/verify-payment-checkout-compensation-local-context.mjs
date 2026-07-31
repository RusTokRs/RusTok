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

const paths = {
  lib: "crates/rustok-payment/src/lib.rs",
  api: "crates/rustok-payment/src/checkout_compensation_api.rs",
  wrapper: "crates/rustok-payment/src/checkout_compensation_context.rs",
  owner: "crates/rustok-payment/src/checkout_compensation.rs",
  commerce: "crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs",
  doc: "crates/rustok-payment/docs/checkout-compensation-local-context.md",
  paymentPlan: "crates/rustok-payment/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  wrapperEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json",
  wrapperReview:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json",
  ownerEvidence:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source.json",
  ownerReview:
    "crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source-review.json",
  wrapperGuard:
    "scripts/verify/verify-payment-checkout-compensation-wrapper-error-diagnostic-safety.mjs",
  ownerGuard:
    "scripts/verify/verify-payment-checkout-compensation-owner-payload-diagnostic-safety.mjs",
};

const lib = read(paths.lib);
const api = read(paths.api);
const wrapper = read(paths.wrapper);
const owner = read(paths.owner);
const commerce = read(paths.commerce);
const doc = read(paths.doc);
const paymentPlan = read(paths.paymentPlan);
const commercePlan = read(paths.commercePlan);
const wrapperEvidence = JSON.parse(read(paths.wrapperEvidence));
const wrapperReview = JSON.parse(read(paths.wrapperReview));
const ownerEvidence = JSON.parse(read(paths.ownerEvidence));
const ownerReview = JSON.parse(read(paths.ownerReview));
const wrapperGuard = read(paths.wrapperGuard);
const ownerGuard = read(paths.ownerGuard);

for (const [value, label] of [
  ['#[path = "checkout_compensation.rs"]\nmod checkout_compensation_persistent;', "private owner module"],
  ['#[path = "checkout_compensation_api.rs"]\npub mod checkout_compensation;', "public facade module"],
  ["mod checkout_compensation_context;", "private wrapper module"],
  ["pub use checkout_compensation::{", "root facade export"],
  ["CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,", "root contracts"],
  ["InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,", "root wrapper construction"],
]) requireText(lib, value, label);
for (const forbidden of [
  "pub mod checkout_compensation_persistent",
  "pub use checkout_compensation_persistent::",
  "pub use checkout_compensation_context::",
]) forbidText(lib, forbidden, "public owner bypass");

for (const [value, label] of [
  ["pub use crate::checkout_compensation_context::{", "module wrapper export"],
  ["InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,", "module wrapper type/factory"],
  ["pub use crate::checkout_compensation_persistent::{", "module owner contract export"],
  ["CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,", "module contracts"],
]) requireText(api, value, label);
for (const forbidden of [
  "PersistentCheckoutPaymentCompensationPort",
  "checkout_compensation_persistent::InProcessCheckoutPaymentCompensationPort",
  "checkout_compensation_persistent::in_process_checkout_payment_compensation_port",
]) forbidText(api, forbidden, "public persistent implementation exposure");

for (const marker of [
  "inner: PersistentCheckoutPaymentCompensationPort",
  "PersistentCheckoutPaymentCompensationPort::new(db)",
  "PersistentCheckoutPaymentCompensationPort::with_provider_registry(",
  "let diagnostic_context = context.clone();",
  "let diagnostic_facts = checkout_payment_compensation_diagnostic_facts(&request);",
  ".compensate_checkout_payment(context, request)",
  "checkout_payment_compensation_local_operation(error.code.as_str())",
  "let error_facts = checkout_payment_compensation_port_error_facts(&error);",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "error_kind = error_facts.error_kind",
  "boundary = PAYMENT_COMPENSATION_BOUNDARY",
  "\n    error\n}",
]) requireText(wrapper, marker, `${paths.wrapper}: wrapper contract`);
for (const forbidden of [
  "error = ?error",
  "internal_message = %error.message",
  "error_kind = ?error.kind",
  "tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "channel = ?context.channel",
  "locale = %context.locale",
  "causation_id = ?context.causation_id",
  "traceparent = ?context.traceparent",
  "idempotency_key = ?context.idempotency_key",
]) forbidText(wrapper, forbidden, `${paths.wrapper}: wrapper payload safety`);

for (const marker of [
  'const PAYMENT_OWNER: &str = "rustok_payment";',
  'const PAYMENT_COMPENSATION_BOUNDARY: &str = "checkout_payment_compensation_port";',
  "struct CheckoutPaymentCompensationOwnerContextFacts",
  "struct CheckoutPaymentCompensationPaymentErrorFacts",
  "enum CheckoutPaymentCompensationOwnerFailureKind",
  "fn checkout_payment_compensation_owner_context_facts(",
  "fn checkout_payment_compensation_payment_error_facts(",
  "fn log_checkout_payment_compensation_owner_failure(",
  "fn log_checkout_payment_compensation_payment_error(",
  "fn log_checkout_payment_compensation_static_error(",
  "fn log_checkout_payment_compensation_owner_warning(",
  "fn log_checkout_payment_compensation_context_warning(",
  "payment_error_variant = ?payment_error_variant",
  "payment_error_text_field_count = ?payment_error_text_field_count",
  "payment_error_uuid_non_nil_count = ?payment_error_uuid_non_nil_count",
  "payment_error_opaque_payload_present = ?payment_error_opaque_payload_present",
  "tenant_id_parse_failed = true",
  "reconciliation_reason_present,",
  "reconciliation_reason_length,",
  "checkout_operation_id_non_nil = ?checkout_operation_id_non_nil",
  "causation_matches = ?causation_matches",
  "owner = PAYMENT_OWNER",
  "correlation_id = %context.correlation_id",
  "boundary = PAYMENT_COMPENSATION_BOUNDARY",
]) requireText(owner, marker, `${paths.owner}: bounded owner source`);

for (const [operation, code] of [
  ["commit_recovered_cancel_checkpoint", "payment.checkout_compensation_commit_checkpoint_failed"],
  ["encode_provider_cancel_request", "payment.checkout_compensation_encoding_failed"],
  ["checkpoint_provider_cancel_failure", "payment.checkout_compensation_provider_failure_checkpoint_failed"],
  ["encode_provider_cancel_result", "payment.checkout_compensation_provider_result_encoding_failed"],
  ["checkpoint_provider_cancel_success", "payment.checkout_compensation_provider_checkpoint_failed"],
  ["commit_provider_cancel_checkpoint", "payment.checkout_compensation_commit_checkpoint_failed"],
  ["decode_provider_cancel_checkpoint", "payment.provider_invalid_response"],
  ["validate_causation_context", "payment.checkout_compensation_causation_invalid"],
  ["parse_tenant_context", "payment.tenant_id_invalid"],
  ["map_payment_owner_error", "payment checkout compensation owner operation failed"],
]) {
  requireText(owner, `"${operation}"`, `${paths.owner}: local operation ${operation}`);
  requireText(owner, `"${code}"`, `${paths.owner}: stable code/event ${code}`);
}

for (const forbidden of [
  "fn log_checkout_payment_compensation_owner_error<",
  "fn log_checkout_payment_compensation_owner_warning<",
  "error = ?error",
  "error = %error",
  "error = ?checkpoint_error",
  "error = %checkpoint_error",
  "\n        internal_message,\n",
  "tenant_id = %context.tenant_id",
  "internal_tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "channel = ?context.channel",
  "locale = %context.locale",
  "causation_id = ?context.causation_id",
  "traceparent = ?context.traceparent",
  "idempotency_key = ?context.idempotency_key",
  "checkout_operation_id = %checkout_operation_id",
  "collection_id = %",
  "collection_id = ?",
  "operation_id = %",
  "provider_id = %",
  "reason = ?",
  "reason = %",
  "metadata = ?",
  "metadata = %",
  "request_amount =",
]) forbidText(owner, forbidden, `${paths.owner}: raw owner payload`);

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

for (const marker of [
  "pub trait CheckoutPaymentCompensationPort",
  "pub struct CheckoutPaymentCompensationRequest",
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
  "parse_tenant_id(&context, owner_operation)?;",
  "require_operation_context(&context, owner_operation, request.checkout_operation_id)?;",
  "let Some(collection_id) = request.collection_id else {\n            return Ok(None);",
  "PaymentCollectionStatusKind::Cancelled",
  "PaymentCollectionStatusKind::Captured",
  "PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized",
  "PaymentCollectionStatusKind::Unknown",
  'format!("payment_collection:{}:cancel", collection.id)',
  '"operation": "cancel_payment_collection"',
  ".execute_cancel(provider_id.as_str(), provider_request)",
  ".begin(BeginProviderOperation {",
  ".claim_execution(operation.id)",
  "persisted_cancel_result(context, owner_operation, &operation)",
  ".mark_provider_succeeded(",
  ".mark_reconciliation_required(operation.id, code)",
  ".mark_provider_error(operation.id, code)",
  ".cancel_local_collection(",
  ".mark_committed(outcome.operation_id)",
  '"payment.checkout_compensation_manual_reconciliation"',
  '"payment checkout compensation requires manual reconciliation"',
  "PaymentError::Database(_) => PortError::unavailable(",
  "PaymentError::Validation(_) => PortError::validation(",
  "PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(",
  "PaymentError::InvalidTransition { .. } => PortError::conflict(",
  "PaymentError::ProviderUnavailable { .. } => PortError::unavailable(",
  "PaymentError::ProviderRejected { .. } => PortError::conflict(",
  "PaymentError::ProviderInvalidResponse { .. } => PortError::invariant_violation(",
  "PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(",
]) requireText(owner, marker, `${paths.owner}: preserved owner behavior`);

for (const marker of [
  "use rustok_payment::{",
  "CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,",
  "InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind, PaymentProviderRegistry,",
  "in_process_checkout_payment_compensation_port,",
]) requireText(commerce, marker, `${paths.commerce}: owner port composition`);
forbidText(
  commerce,
  "rustok_payment::checkout_compensation_persistent::",
  `${paths.commerce}: persistent bypass`,
);

for (const [pathLabel, evidence, expectedStatus] of [
  [paths.wrapperEvidence, wrapperEvidence, "payment_checkout_compensation_wrapper_diagnostic_safety_source_unvalidated"],
  [paths.wrapperReview, wrapperReview, "payment_checkout_compensation_wrapper_diagnostic_safety_source_reviewed_unvalidated"],
  [paths.ownerEvidence, ownerEvidence, "payment_checkout_compensation_owner_diagnostic_safety_source_unvalidated"],
  [paths.ownerReview, ownerReview, "payment_checkout_compensation_owner_diagnostic_safety_source_reviewed_unvalidated"],
]) {
  if (evidence.status !== expectedStatus) {
    failures.push(`${pathLabel}: unexpected status ${evidence.status}`);
  }
}

for (const [pathLabel, contract] of [
  [paths.wrapperEvidence, wrapperEvidence.source_contract],
  [paths.wrapperReview, wrapperReview.review_findings],
  [paths.ownerEvidence, ownerEvidence.source_contract],
  [paths.ownerReview, ownerReview.review_findings],
]) {
  if (contract?.persistent_owner_diagnostic_cleanup_complete !== true) {
    failures.push(`${pathLabel}: persistent owner diagnostic cleanup must be true`);
  }
  if (contract?.checkout_compensation_payload_diagnostic_cleanup_closed !== true) {
    failures.push(`${pathLabel}: compensation payload diagnostics must be source-closed`);
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
  if (ownerEvidence.validation?.[key] !== false) {
    failures.push(`${paths.ownerEvidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Human-readable `PortError.message` is not used as control flow.",
  "Five `PaymentError` event sites retain only:",
  "Three codec event sites retain only static",
  "payload-diagnostic sites at source level",
  "Compile, provider replay, process-exit, restart",
  "No FBA or FFA status is promoted from source inspection.",
]) requireText(doc, marker, `${paths.doc}: compensation documentation`);
requireText(
  paymentPlan,
  "The currently identified checkout compensation payload-diagnostic sites are source-closed",
  `${paths.paymentPlan}: payment owner status`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup for order, payment execution/compensation,",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);
requireText(
  wrapperGuard,
  "Payment checkout compensation wrapper error diagnostic-safety verification failed:",
  `${paths.wrapperGuard}: focused wrapper guard`,
);
requireText(
  ownerGuard,
  "Payment checkout compensation owner payload diagnostic-safety verification failed:",
  `${paths.ownerGuard}: focused owner guard`,
);

if (failures.length > 0) {
  console.error("Payment checkout compensation local-context verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout compensation wrapper and persistent owner use bounded payload diagnostics while preserving owner execution and public envelopes; runtime evidence remains open",
);
