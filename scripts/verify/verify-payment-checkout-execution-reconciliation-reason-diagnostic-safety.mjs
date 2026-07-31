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
  validation: "crates/rustok-payment/src/checkout_execution/validation_errors.rs",
  helpers: "crates/rustok-payment/src/checkout_execution/provider_helpers.rs",
  authorize: "crates/rustok-payment/src/checkout_execution/prepare_authorize.rs",
  capture: "crates/rustok-payment/src/checkout_execution/capture_provider.rs",
  evidence:
    "crates/rustok-payment/contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json",
  doc:
    "crates/rustok-payment/docs/checkout-execution-reconciliation-reason-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const validation = read(paths.validation);
const helpers = read(paths.helpers);
const authorize = read(paths.authorize);
const capture = read(paths.capture);
const runtime = [validation, helpers, authorize, capture].join("\n");
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const variants = [
  ["MissingNormalizedDurableResult", "missing_normalized_durable_result"],
  ["MalformedDurableResult", "malformed_durable_result"],
  ["InvalidSuccessfulProviderResponse", "invalid_successful_provider_response"],
  ["UnknownProviderOutcome", "unknown_provider_outcome"],
  ["MissingDurableAuthorizeProviderIdentity", "missing_durable_authorize_provider_identity"],
  ["IncompleteAuthorizeOperation", "incomplete_authorize_operation"],
  ["MissingDurableProviderPaymentIdentity", "missing_durable_provider_payment_identity"],
  ["CommitCheckpointFailed", "commit_checkpoint_failed"],
  ["UnknownCollectionLifecycleBeforeAuthorization", "unknown_collection_lifecycle_before_authorization"],
  ["AuthorizationLocalPersistenceIncomplete", "authorization_local_persistence_incomplete"],
  ["UnknownCollectionLifecycleBeforeCapture", "unknown_collection_lifecycle_before_capture"],
  ["CaptureLocalPersistenceIncomplete", "capture_local_persistence_incomplete"],
  ["ProviderOperationInProgressOrReconciliationRequired", "provider_operation_in_progress_or_reconciliation_required"],
  ["ProviderFailureCheckpointFailed", "provider_failure_checkpoint_failed"],
  ["ProviderResultEncodingFailed", "provider_result_encoding_failed"],
  ["ProviderSuccessCheckpointFailed", "provider_success_checkpoint_failed"],
];

requireText(
  validation,
  "enum CheckoutPaymentExecutionReconciliationReason {",
  `${paths.validation}: closed reconciliation enum`,
);
requireText(
  validation,
  "impl CheckoutPaymentExecutionReconciliationReason {",
  `${paths.validation}: reconciliation labels`,
);
requireText(
  validation,
  "fn label(self) -> &'static str",
  `${paths.validation}: static label type`,
);

for (const [variant, label] of variants) {
  requireText(validation, `${variant},`, `${paths.validation}: ${variant} variant`);
  requireText(
    validation,
    `Self::${variant}`,
    `${paths.validation}: ${variant} label arm`,
  );
  requireText(validation, `"${label}"`, `${paths.validation}: ${label} label`);
  requireCount(
    runtime,
    `CheckoutPaymentExecutionReconciliationReason::${variant}`,
    1,
    `runtime: one ${variant} call site`,
  );
}

const helper = functionBody(validation, "manual_reconciliation");
for (const marker of [
  "reason: CheckoutPaymentExecutionReconciliationReason",
  "let reconciliation_reason = reason.label();",
  "reconciliation_reason,",
  "correlation_id = %context.correlation_id",
  "operation = owner_operation",
  'code = "payment.checkout_execution_manual_reconciliation"',
  "boundary = PAYMENT_EXECUTION_BOUNDARY",
  "PortError::new(",
  "PortErrorKind::Conflict",
  '"payment checkout execution requires manual reconciliation"',
  "false,",
]) {
  requireText(helper, marker, `${paths.validation}: reconciliation helper`);
}
for (const forbidden of [
  "internal_message",
  "reason: &'static str",
  "reason: &str",
  "reason = %",
  "reason = ?",
]) {
  forbidText(helper, forbidden, `${paths.validation}: free-form reason diagnostics`);
}

requireCount(runtime, "manual_reconciliation(", 17, "runtime: helper plus sixteen call sites");

for (const oldReason of [
  "payment provider operation has no normalized durable result",
  "payment provider operation result is malformed",
  "payment provider returned an invalid successful response",
  "payment provider operation outcome is unknown",
  "payment capture has no durable authorize provider identity",
  "payment capture cannot use an incomplete authorize operation",
  "payment authorize operation has no durable provider payment identity",
  "payment provider result was applied but its commit checkpoint failed",
  "payment collection lifecycle is unknown before authorization",
  "payment authorization succeeded externally but local persistence is incomplete",
  "payment collection lifecycle is unknown before capture",
  "payment capture succeeded externally but local persistence is incomplete",
  "payment provider operation is already executing or requires reconciliation",
  "payment provider failure could not be durably checkpointed",
  "payment provider succeeded but its normalized result could not be persisted",
  "payment provider succeeded but its durable checkpoint failed",
]) {
  forbidText(runtime, `"${oldReason}",`, `runtime: free-form reason ${oldReason}`);
}

if (
  evidence.status !==
  "payment_checkout_execution_reconciliation_reason_diagnostic_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  reconciliation_reason_variant_count: 16,
  manual_reconciliation_call_site_count: 16,
  all_checkout_execution_call_sites_typed: true,
  free_form_reason_parameter_allowed: false,
  free_form_reason_text_logged: false,
  stable_reason_label_logged: true,
  durable_result_routes_typed: true,
  owner_error_routes_typed: true,
  provider_identity_routes_typed: true,
  authorize_lifecycle_and_persistence_routes_typed: true,
  capture_lifecycle_and_persistence_routes_typed: true,
  provider_journal_checkpoint_and_encoding_routes_typed: true,
  manual_reconciliation_code_preserved: true,
  manual_reconciliation_kind_preserved: true,
  manual_reconciliation_message_preserved: true,
  manual_reconciliation_retryability_preserved: true,
  call_routing_preserved: true,
  provider_result_recovery_changed: false,
  provider_execution_changed: false,
  payment_lifecycle_changed: false,
  local_persistence_diagnostics_changed: true,
  provider_checkpoint_diagnostics_changed: false,
  remaining_provider_checkpoint_diagnostics_open: true,
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
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "closed enum contains sixteen reasons", `${paths.doc}: reason count`);
requireText(doc, "All sixteen checkout execution call sites", `${paths.doc}: call coverage`);
requireText(
  doc,
  "Separate cleanup remains open for provider checkpoint",
  `${paths.doc}: remaining work`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error(
    "Payment checkout execution reconciliation-reason diagnostic-safety verification failed:",
  );
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Payment checkout execution reconciliation diagnostics use sixteen stable typed reasons across all call sites; execution evidence remains open",
);
