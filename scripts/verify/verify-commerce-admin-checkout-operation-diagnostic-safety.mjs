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
  source: "crates/rustok-commerce/src/controllers/admin/checkout_operations.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-checkout-operation-diagnostic-safety.md",
  broadVerifier: "scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function blockBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const broadVerifier = read(paths.broadVerifier);
const plan = read(paths.plan);

const rawContext = blockBetween(
  source,
  "struct AdminCheckoutOperationErrorContext {",
  "impl AdminCheckoutOperationErrorContext {",
  "raw error context",
);
for (const marker of [
  "tenant_id: Uuid",
  "actor_id: Uuid",
  "checkout_operation_id: Option<Uuid>",
  "reservation_id: Option<Uuid>",
  "payment_collection_id: Option<Uuid>",
  "payment_id: Option<Uuid>",
  "refund_id: Option<Uuid>",
  "order_id: Option<Uuid>",
  "order_return_id: Option<Uuid>",
  "order_change_id: Option<Uuid>",
  "operation: &'static str",
]) requireText(rawContext, marker, `${paths.source}: internal context preserved`);

const diagnosticContext = blockBetween(
  source,
  "struct AdminCheckoutOperationDiagnosticContext {",
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "bounded diagnostic context",
);
for (const field of [
  "tenant_id",
  "actor_id",
  "checkout_operation_id",
  "reservation_id",
  "payment_collection_id",
  "payment_id",
  "refund_id",
  "order_id",
  "order_return_id",
  "order_change_id",
  "operation",
]) requireText(diagnosticContext, `${field}: &'static str`, `${paths.source}: bounded ${field}`);
for (const forbidden of ["Uuid", "Option<", "String"]) {
  forbidText(diagnosticContext, forbidden, `${paths.source}: bounded context storage`);
}

const conversion = blockBetween(
  source,
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "fn uuid_shape(",
  "diagnostic conversion",
);
for (const marker of [
  "tenant_id: uuid_shape(context.tenant_id)",
  "actor_id: uuid_shape(context.actor_id)",
  "checkout_operation_id: optional_uuid_shape(context.checkout_operation_id)",
  "reservation_id: optional_uuid_shape(context.reservation_id)",
  "payment_collection_id: optional_uuid_shape(context.payment_collection_id)",
  "payment_id: optional_uuid_shape(context.payment_id)",
  "refund_id: optional_uuid_shape(context.refund_id)",
  "order_id: optional_uuid_shape(context.order_id)",
  "order_return_id: optional_uuid_shape(context.order_return_id)",
  "order_change_id: optional_uuid_shape(context.order_change_id)",
  "operation: context.operation",
]) requireText(conversion, marker, `${paths.source}: diagnostic conversion`);

const requiredShape = blockBetween(
  source,
  "fn uuid_shape(",
  "fn optional_uuid_shape(",
  "required UUID shape",
);
for (const marker of ["value.is_nil()", '"nil"', '"non_nil"']) {
  requireText(requiredShape, marker, `${paths.source}: required UUID shape`);
}

const optionalShape = blockBetween(
  source,
  "fn optional_uuid_shape(",
  "#[derive(Clone, Debug, Serialize, ToSchema)]",
  "optional UUID shape",
);
for (const marker of [
  "None => \"absent\"",
  "Some(value) if value.is_nil() => \"present_nil\"",
  "Some(_) => \"present_non_nil\"",
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

const logger = blockBetween(
  source,
  "fn admin_checkout_operation_http_error<E>(",
  "fn map_operation_error(",
  "admin checkout logger",
);
requireOrder(
  logger,
  [
    "let _ = error;",
    "let context = AdminCheckoutOperationDiagnosticContext::from(context);",
    'let error = "redacted";',
    "let (status, code, message, error_kind) = policy;",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: bounded logger order`,
);
for (const marker of [
  "error = ?error",
  "owner = ADMIN_CHECKOUT_OPERATION_OWNER",
  "source_owner,",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
  "reservation_id = ?context.reservation_id",
  "payment_collection_id = ?context.payment_collection_id",
  "payment_id = ?context.payment_id",
  "refund_id = ?context.refund_id",
  "order_id = ?context.order_id",
  "order_return_id = ?context.order_return_id",
  "order_change_id = ?context.order_change_id",
  "operation = %context.operation",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY",
]) requireText(logger, marker, `${paths.source}: retained bounded field`);
for (const forbidden of [
  "where\n    E: std::fmt::Debug",
  "format!(",
  ".to_string()",
  "error.message",
]) forbidText(logger, forbidden, `${paths.source}: raw logger payload`);

for (const marker of [
  "CheckoutOperationError::NotFound(_)",
  "CheckoutOperationError::Conflict(_)",
  "CheckoutOperationError::Validation(_)",
  "CheckoutOperationError::Database(_)",
  "adopt_operation_error_identity(&mut context, &error)",
  "adopt_reservation_error_identity(&mut context, source)",
  "CheckoutCompensationError::ManualReconciliation(_)",
  "CheckoutCompensationError::Conflict(_)",
  "CheckoutCompensationError::Boundary {",
  "CheckoutCompensationError::CompensationAndJournal { .. }",
  "HttpError::new(status, code, message)",
]) requireText(source, marker, `${paths.source}: preserved mapping behavior`);

for (const marker of [
  "error = ?error",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
  "HttpError::new(status, code, message)",
]) requireText(broadVerifier, marker, `${paths.broadVerifier}: compatibility marker`);

if (
  evidence.status !==
  "commerce_admin_checkout_operation_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_debug_error_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_optional_domain_uuid_logged: false,
  error_redacted_marker_logged: true,
  required_uuid_shape_logged: true,
  optional_uuid_shape_logged: true,
  existing_log_site_markers_preserved: true,
  operation_policy_preserved: true,
  compensation_policy_preserved: true,
  identity_adoption_preserved: true,
  http_envelopes_preserved: true,
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

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "Required tenant and actor UUIDs are represented only as `nil` or `non_nil`.",
  "The typed error is replaced in the event by the stable marker `redacted`.",
  "The broader ecommerce correlation-safe mapper task remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin checkout-operation diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin checkout-operation diagnostics use bounded UUID shapes and a redacted error marker; execution validation remains open",
);
