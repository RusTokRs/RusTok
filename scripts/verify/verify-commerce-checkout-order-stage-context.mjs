#!/usr/bin/env node
/**
 * Checkout order stage error safety.
 *
 * Commerce checkout reads, creates, and confirms the order behind a staged
 * checkout operation. Every Order owner failure is projected onto a bounded
 * vocabulary: a stable Commerce code, a static consumer message, and the owner
 * retryability decision. Owner text survives only as shape and length facts on a
 * redacted tracing event, and the staged-checkout journal persists only
 * Commerce-authored text.
 *
 * This guard pins that contract in the source, the design note, and the source
 * review evidence, and it fails if the retired compatibility sources return.
 */
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (file) => readFileSync(new URL(file, root), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const stage = read("crates/modules/rustok-commerce/src/services/checkout_order_stages.rs");
const ownerFailure = read(
  "crates/modules/rustok-commerce/src/services/checkout_order_owner_failure.rs",
);
const creation = read(
  "crates/modules/rustok-commerce/src/services/checkout_order_creation.rs",
);
const confirmation = read(
  "crates/modules/rustok-commerce/src/services/checkout_order_confirmation.rs",
);
const staged = read("crates/modules/rustok-commerce/src/services/staged_checkout.rs");
const doc = read("crates/modules/rustok-commerce/docs/checkout-order-stage-context.md");
const evidence = JSON.parse(
  read(
    "crates/modules/rustok-commerce/contracts/evidence/checkout-order-stage-error-safety-source-review.json",
  ),
);

for (const retired of [
  "crates/modules/rustok-commerce/src/services/checkout_order_stages_legacy.rs",
  "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline.rs",
]) {
  if (existsSync(new URL(retired, root))) {
    failures.push(`${retired}: retired source must not return`);
  }
}

for (const [value, label] of [
  [".read_checkout_order(", "owner projection read"],
  ["ReadCheckoutOrderProjectionRequest {", "owner projection read request"],
  ["fn order_read_context(", "bounded read context"],
  ["PortActor::system()", "internal continuation actor"],
  ["fn owner_read_failure(", "order-stage owner sanitization"],
  ["CheckoutOrderBoundaryFailure::from_port_error(error)", "port failure projection"],
  ["failure.observe_port_error(context, owner_operation, stage)", "bounded port diagnostics"],
  ["Self::Boundary {", "retained boundary variant"],
  ["code: failure.code", "owner code propagation"],
  ["message: failure.message.to_string()", "static message propagation"],
  ["retryable: failure.retryable", "owner retryability propagation"],
  ["pub fn bounded_failure(&self) -> Option<(&str, bool)>", "bounded failure accessor"],
]) requireText(stage, value, label);

for (const [value, label] of [
  ["include!(", "retired include mount"],
  ["CheckoutCompletionPort", "retired completion write path"],
  ["OrderService", "direct owner service"],
  ["{err}", "raw owner error interpolation"],
  ["{error}", "raw error interpolation"],
  ["error = ?error", "raw owner error logging"],
  ["tenant_id = %context.tenant_id", "raw tenant logging"],
  ["correlation_id = %context.correlation_id", "raw correlation logging"],
  ["message: error.message", "raw owner message propagation"],
]) forbidText(stage, value, label);

for (const [value, label] of [
  ["pub(crate) const CHECKOUT_ORDER_OWNER_BOUNDARY", "adapter boundary"],
  ["pub(crate) struct CheckoutOrderDiagnosticError", "redacted diagnostic token"],
  ['formatter.write_str("redacted")', "redacted debug output"],
  ["pub(crate) fn from_owner_error(error: &OrderError) -> Self", "owner error projection"],
  ["pub(crate) fn from_port_error(error: PortError) -> Self", "port error projection"],
  ["fn of_owner_error(error: &OrderError) -> Self", "typed owner classification"],
  ["fn of_port_error(kind: &PortErrorKind) -> Self", "typed port classification"],
  ["pub(crate) fn observe_owner_error(", "owner error diagnostics"],
  ["pub(crate) fn observe_port_error(", "port error diagnostics"],
  ["owner_message_shape", "owner message shape"],
  ["owner_message_len", "owner message length"],
  ["owner_kind = self.kind.label()", "owner kind"],
  ["owner_code = %self.code", "owner code"],
  ["owner_retryable = self.retryable", "owner retryability"],
  ["fn is_technical(self) -> bool", "severity classification"],
  ["Checkout order request is invalid", "validation public message"],
  ["Checkout order resource was not found", "not-found public message"],
  ["Checkout order state conflicts with the requested operation", "conflict public message"],
  ["Checkout order operation is not permitted", "forbidden public message"],
  ["Checkout order service is temporarily unavailable", "availability public message"],
  ["Checkout order operation could not be completed safely", "invariant public message"],
  ["checkout.order_request_invalid", "validation code"],
  ["checkout.order_resource_not_found", "not-found code"],
  ["checkout.order_state_conflict", "conflict code"],
  ["checkout.order_temporarily_unavailable", "availability code"],
  ["checkout.order_receipt_corrupt", "receipt code"],
  ["checkout.order_operation_failed", "core code"],
  ["impl From<&PortContext> for CheckoutOrderContextFacts", "bounded context projection"],
]) requireText(ownerFailure, value, label);

for (const [value, label] of [
  ["message = %", "raw owner message diagnostic"],
  ["error = ?error", "raw owner error diagnostic"],
  ["error = %error", "raw owner error diagnostic"],
]) forbidText(ownerFailure, value, label);

for (const [source, label] of [
  [creation, "order creation executor"],
  [confirmation, "order confirmation executor"],
]) {
  for (const [value, marker] of [
    ["OrderBoundary {", "bounded owner failure variant"],
    ["fn owner_failure(", "owner failure projection"],
    ["CheckoutOrderBoundaryFailure::from_owner_error(&error)", "owner error classification"],
    ["failure.observe_owner_error(", "bounded owner diagnostics"],
    ["pub fn bounded_failure(&self) -> Option<(&str, bool)>", "bounded failure accessor"],
  ]) requireText(source, value, `${label} ${marker}`);
  for (const [value, marker] of [
    ["Order(#[from] OrderError)", "transparent owner error"],
    ["{err}", "raw owner error interpolation"],
    ["{error}", "raw error interpolation"],
  ]) forbidText(source, value, `${label} ${marker}`);
}

for (const [value, label] of [
  ["fn pipeline_failure_message(error: &CheckoutStagePipelineError) -> String", "bounded journal text"],
  ["pipeline_failure_message(&pipeline)", "bounded journal persistence"],
  ["fn pipeline_failure_stage(error: &CheckoutStagePipelineError) -> &'static str", "stage label fallback"],
  ["checkout pipeline failed at stage", "static fallback sentence"],
  ["CheckoutStagePipelineError::OrderStage(error) => error", "order-stage code propagation"],
  ["checkout.order_stage_failed", "order-stage code fallback"],
  ["bounded_failure()", "bounded retryability propagation"],
]) requireText(staged, value, label);

forbidText(staged, "pipeline.to_string()", "raw pipeline text persistence");

for (const [value, label] of [
  ["Status: **source-reviewed / unvalidated**", "truthful document status"],
  ["## Retired sources", "retired source section"],
  ["Checkout order service is temporarily unavailable", "documented availability message"],
  ["checkout pipeline failed at stage", "documented journal fallback"],
  ["## Remaining work", "open work disclosure"],
  ["cargo check -p rustok-commerce --lib", "maintainer compile check"],
]) requireText(doc, value, label);

for (const [key, expected] of Object.entries({
  mounted_facade_active: true,
  legacy_source_retired: true,
  dead_pipeline_duplicate_retired: true,
  completion_port_writes_retired: true,
  order_service_calls_removed_from_stage: true,
  order_projection_read_uses_owner_port: true,
  owner_message_public: false,
  owner_message_persisted: false,
  raw_owner_error_logged: false,
  raw_context_values_logged: false,
  bounded_context_shapes_logged: true,
  owner_message_shape_logged: true,
  owner_message_length_logged: true,
  owner_code_preserved: true,
  owner_kind_preserved: true,
  owner_retryability_preserved: true,
  order_stage_bounded_code_propagated: true,
  order_stage_retryability_propagated: true,
  journal_persists_bounded_text: true,
  successful_dtos_changed: false,
  checkpoint_order_changed: false,
  idempotency_identity_changed: false,
  commerce_ffa_status_changed: false,
  commerce_fba_status_changed: false,
  broad_mapper_cleanup_closed: false,
  transport_display_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}

if (failures.length > 0) {
  console.error("Checkout order stage error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Checkout order read, creation, and confirmation owner failures are sanitized before the mounted stage mapper and the operation journal",
);
