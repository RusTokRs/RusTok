#!/usr/bin/env node
/**
 * Checkout payment stage error safety.
 *
 * Commerce owns the checkout stage, rustok-payment owns the money. The stage
 * reaches the owner only through `CheckoutPaymentExecutionPort`, and every owner
 * rejection crosses a sanitizing adapter that keeps the owner code and
 * retryability, replaces the owner message with a static kind-based consumer
 * message, and logs bounded facts instead of raw context values.
 *
 * The compatibility source `checkout_payment_stages_legacy.rs` and the shim
 * modules that existed only to mount it are retired; this guard fails if either
 * returns, because a re-mounted legacy source would bypass the sanitizer.
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

const facade = read("crates/modules/rustok-commerce/src/services/checkout_payment_stages.rs");
const staged = read("crates/modules/rustok-commerce/src/services/staged_checkout.rs");
const evidence = JSON.parse(
  read(
    "crates/modules/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json",
  ),
);

for (const retired of [
  "crates/modules/rustok-commerce/src/services/checkout_payment_stages_legacy.rs",
  "crates/modules/rustok-commerce/src/services/checkout_fulfillment_stages_legacy.rs",
]) {
  if (existsSync(new URL(retired, root))) {
    failures.push(`${retired}: retired source must not return`);
  }
}

for (const [value, label] of [
  ["mod payment_execution_boundary {", "bounded mapper module"],
  ["mod payment_execution_port {", "sanitizing owner port module"],
  ["struct SanitizingCheckoutPaymentExecutionPort", "sanitizing owner adapter"],
  ["struct BoundaryPortError", "private bounded error"],
  ['formatter.write_str("redacted")', "redacted diagnostic debug"],
  ["fn public_message(kind: &PortErrorKind)", "typed public message policy"],
  ["fn sanitize_owner_error(", "owner mapper"],
  ["CheckoutPaymentExecutionContextFacts", "bounded context facts"],
  ["message: public_message.to_string()", "static stage message"],
  ["code: error.code", "owner code preservation"],
  ["retryable: error.retryable", "owner retryability preservation"],
  ["wrap_checkout_payment_execution_port(payment_port)", "custom port adapter"],
  ["pub struct CheckoutPaymentStageExecutor", "public stage executor"],
  ["advance_to_payment_captured(", "advance API"],
  ["load_payment_captured_state(", "recovery API"],
  [".map_err(|error| payment_boundary_error(\"prepare\", error))", "prepare boundary mapping"],
  [".map_err(|error| payment_boundary_error(\"authorize\", error))", "authorize boundary mapping"],
  [".map_err(|error| payment_boundary_error(\"capture\", error))", "capture boundary mapping"],
  [".map_err(|error| payment_boundary_error(\"read\", error))", "read boundary mapping"],
  ["message: public_message(&error.kind).to_string()", "static stage message derivation"],
  ["next_stage: CheckoutOperationStage::PaymentAuthorized", "authorize checkpoint"],
  ["next_stage: CheckoutOperationStage::PaymentCaptured", "capture checkpoint"],
  ['format!("payment_collection:{}:authorize", collection.id)', "authorize idempotency"],
  ['format!("payment_collection:{collection_id}:capture")', "capture idempotency"],
]) requireText(facade, value, label);

for (const operation of [
  "prepare_checkout_collection",
  "authorize_checkout_collection",
  "capture_checkout_collection",
  "read_checkout_collection",
]) {
  requireText(facade, operation, `${operation} bounded operation`);
}

for (const [value, label] of [
  ["message: error.message", "raw owner message propagation"],
  ["error = ?boundary_error", "raw owner error diagnostic"],
  ["tenant_id = %context.tenant_id", "raw tenant diagnostic"],
  ["actor = ?context.actor", "raw actor diagnostic"],
  ["channel = ?context.channel", "raw channel diagnostic"],
  ["locale = %context.locale", "raw locale diagnostic"],
  ["correlation_id = %context.correlation_id", "raw correlation diagnostic"],
  ["idempotency_key = ?context.idempotency_key", "raw idempotency diagnostic"],
  ["include!(", "retired compatibility mount"],
  ["mod rustok_api_shim {", "retired API shim"],
  ["mod rustok_payment_shim {", "retired owner shim"],
  ["mod tracing_shim {", "retired tracing shim"],
  ["use super::tracing_shim as tracing;", "retired tracing shadow"],
]) forbidText(facade, value, label);

for (const [value, label] of [
  ["CheckoutPaymentStageError", "typed payment stage disposition import"],
  [
    "CheckoutStagePipelineError::PaymentStage(CheckoutPaymentStageError::Boundary {\n            retryable: true,",
    "retryable payment boundary disposition",
  ],
  ["fn retryable_payment_stage_boundary_does_not_force_compensation()", "retryable disposition source test"],
  ["fn non_retryable_payment_stage_boundary_requires_compensation()", "non-retryable disposition source test"],
]) requireText(staged, value, label);

const requiredEvidence = {
  status: "commerce_checkout_payment_stage_error_safety_source_reviewed_unvalidated",
  mounted_facade_active: true,
  legacy_source_retired: true,
  canonical_port_api_preserved: true,
  owner_context_delegation_preserved: true,
  owner_message_public: false,
  owner_message_persisted: false,
  raw_port_error_logged: false,
  raw_context_values_logged: false,
  owner_code_preserved: true,
  owner_retryability_preserved: true,
  failure_disposition_changed: true,
  retryable_payment_boundary_disposition_retryable: true,
  non_retryable_payment_boundary_disposition_compensation_required: true,
  retryable_payment_boundary_synchronous_compensation: false,
  operation_journal_implementation_changed: false,
  recovery_service_implementation_changed: false,
  payment_stage_execution_changed: true,
  runtime_evidence_claimed: false,
};

if (evidence.status !== requiredEvidence.status) {
  failures.push(`evidence status: expected ${requiredEvidence.status}, got ${evidence.status}`);
}
for (const [key, expected] of Object.entries(requiredEvidence).slice(1)) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence ${key}: expected ${expected}, got ${evidence.source_contract?.[key]}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation ${key}: expected false`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}

if (failures.length > 0) {
  console.error("Checkout payment stage error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Mounted payment errors stay bounded and retryable boundaries remain resumable");
