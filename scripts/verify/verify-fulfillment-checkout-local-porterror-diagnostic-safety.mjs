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
const countText = (source, value) => source.split(value).length - 1;
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};

const source = read("crates/rustok-fulfillment/src/checkout_execution.rs");
const evidence = JSON.parse(
  read(
    "crates/rustok-fulfillment/contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json",
  ),
);
const admissionEvidence = JSON.parse(
  read(
    "crates/rustok-fulfillment/contracts/evidence/checkout-admission-diagnostic-safety-source.json",
  ),
);
const doc = read(
  "crates/rustok-fulfillment/docs/checkout-execution-local-porterror-diagnostic-safety.md",
);

const mapper = between(
  source,
  "fn map_checkout_fulfillment_local_port_error(",
  "pub fn in_process_checkout_fulfillment_execution_port(",
  "checkout fulfillment local PortError mapper",
);
const admission = between(
  source,
  "fn log_checkout_fulfillment_admission_rejection(",
  "#[async_trait]\nimpl CheckoutFulfillmentExecutionPort",
  "checkout fulfillment admission mapper",
);

for (const [value, label] of [
  ["let error_kind = match &error.kind", "closed local PortError kind classification"],
  ['PortErrorKind::Validation => "validation"', "validation kind label"],
  ['PortErrorKind::NotFound => "not_found"', "not-found kind label"],
  ['PortErrorKind::Conflict => "conflict"', "conflict kind label"],
  ['PortErrorKind::Forbidden => "forbidden"', "forbidden kind label"],
  ['PortErrorKind::Unavailable => "unavailable"', "unavailable kind label"],
  ['PortErrorKind::Timeout => "timeout"', "timeout kind label"],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', "invariant kind label"],
  ["let technical_failure = matches!(", "local severity classification"],
  ["let actor_kind = match &context.actor.kind", "bounded actor kind"],
  ["let tenant_id_length = context.tenant_id.chars().count();", "tenant shape"],
  ["let actor_id_length = context.actor.id.chars().count();", "actor shape"],
  ["let claim_count = context.claims.len();", "claim count"],
  ["let role_count = context.roles.len();", "role count"],
  ["let channel_present = context.channel.is_some();", "channel presence"],
  ["let channel_length = context.channel.as_ref()", "channel length"],
  ["let locale_length = context.locale.chars().count();", "locale length"],
  ["let causation_id_present = context.causation_id.is_some();", "causation presence"],
  ["let causation_id_length = context", "causation length"],
  ["let traceparent_present = context.traceparent.is_some();", "traceparent presence"],
  ["let traceparent_length = context", "traceparent length"],
  ["let idempotency_key_present = context.idempotency_key.is_some();", "idempotency presence"],
  ["let idempotency_key_length = context", "idempotency length"],
  ["let internal_message_present = !error.message.trim().is_empty();", "message presence"],
  ["let internal_message_length = error.message.chars().count();", "message length"],
  ["correlation_id = %context.correlation_id", "correlation diagnostic"],
  ["internal_code = %error.code", "stable internal code"],
  ["internal_message_present", "bounded message presence diagnostic"],
  ["internal_message_length", "bounded message length diagnostic"],
  ["error_kind", "closed kind diagnostic"],
  ["retryable = error.retryable", "retryability diagnostic"],
  ["checkout fulfillment local owner operation failed", "technical diagnostic message"],
  ["checkout fulfillment local owner operation was rejected", "rejection diagnostic message"],
  ["\n    error\n}", "original PortError pass-through"],
]) requireText(mapper, value, label);

for (const payload of [
  "error = ?error",
  "error = %error",
  "tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "channel = ?context.channel",
  "locale = %context.locale",
  "causation_id = ?context.causation_id",
  "traceparent = ?context.traceparent",
  "idempotency_key = ?context.idempotency_key",
  "internal_message = %error.message",
  "error_kind = ?error.kind",
]) forbidText(mapper, payload, "complete local error, message, or raw context diagnostic");

if (countText(mapper, "tracing::error!(") !== 1) {
  failures.push("expected exactly one local technical diagnostic path");
}
if (countText(mapper, "tracing::warn!(") !== 1) {
  failures.push("expected exactly one local rejection diagnostic path");
}
for (const marker of [
  "correlation_id = %context.correlation_id",
  "internal_code = %error.code",
  "internal_message_present",
  "internal_message_length",
  "error_kind",
  "retryable = error.retryable",
]) {
  if (countText(mapper, marker) < 2) {
    failures.push(`both local severity paths must retain ${marker}`);
  }
}
if (countText(source, "map_checkout_fulfillment_local_port_error(") !== 9) {
  failures.push("expected one local mapper definition plus eight preserved call sites");
}

for (const [value, label] of [
  ["let error_kind = match &error.kind", "bounded admission kind classification"],
  ["let internal_message_present = !error.message.trim().is_empty();", "bounded admission message presence"],
  ["let internal_message_length = error.message.chars().count();", "bounded admission message length"],
  ["tenant_id_length", "bounded admission tenant shape"],
  ["actor_kind", "bounded admission actor shape"],
  ["channel_present", "bounded admission channel shape"],
  ["correlation_id = %context.correlation_id", "admission correlation"],
  ["internal_code = %error.code", "admission code"],
  ["retryable = error.retryable", "admission retryability"],
]) requireText(admission, value, label);
for (const payload of [
  "error = ?error",
  "error = %error",
  "tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "channel = ?context.channel",
  "locale = %context.locale",
  "causation_id = ?context.causation_id",
  "traceparent = ?context.traceparent",
  "idempotency_key = ?context.idempotency_key",
  "internal_message = %error.message",
  "error_kind = ?error.kind",
]) forbidText(admission, payload, "unsafe admission payload after separate cleanup");

if (evidence.status !== "fulfillment_checkout_local_porterror_diagnostic_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  local_mapper_bounded: true,
  complete_local_port_error_logged: false,
  local_internal_message_text_logged: false,
  local_context_shape_only: true,
  local_correlation_preserved: true,
  local_owner_operations_preserved: true,
  local_error_kind_closed: true,
  local_severity_split_preserved: true,
  original_port_error_returned: true,
  local_mapper_call_sites_preserved: true,
  admission_diagnostic_cleanup_source_closed_separately: true,
  causation_tenant_parser_cleanup_out_of_scope: true,
  canonical_fulfillment_error_mapper_cleanup_out_of_scope: true,
  execution_behavior_changed: false,
  public_port_error_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
if (admissionEvidence.status !== "fulfillment_checkout_admission_diagnostic_safety_source_unvalidated") {
  failures.push(`admission evidence status mismatch: ${admissionEvidence.status}`);
}
if (admissionEvidence.source_contract?.admission_mapper_bounded !== true) {
  failures.push("admission evidence must mark the mapper bounded");
}
if (admissionEvidence.source_contract?.complete_admission_port_error_logged !== false) {
  failures.push("admission evidence must reject complete PortError logging");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

for (const [value, label] of [
  ["Status: **source-ready / unvalidated**", "documentation status"],
  ["The local mapper records only a closed error-kind label", "documentation local error policy"],
  ["The exact delegated `PortError` is returned unchanged", "documentation pass-through policy"],
  ["Admission diagnostics are source-ready / unvalidated under a separate contract", "documentation admission boundary"],
]) requireText(doc, value, label);

if (failures.length > 0) {
  console.error("Fulfillment checkout local PortError diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ fulfillment checkout local and admission PortError diagnostics use bounded kind, message-shape, and context-shape facts under separate source-only contracts",
);
