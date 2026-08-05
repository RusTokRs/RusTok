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
  source: "crates/rustok-commerce/src/controllers/admin/orders.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-order-mutation-diagnostic-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-order-mutation-diagnostic-safety.md",
  broadVerifier: "scripts/verify/verify-commerce-admin-order-route-error-context.mjs",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const actual = source.split(value).length - 1;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
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

const diagnosticContext = blockBetween(
  source,
  "struct AdminOrderMutationDiagnosticContext {",
  "impl From<&AdminOrderErrorContext> for AdminOrderMutationDiagnosticContext {",
  "bounded mutation context",
);
for (const field of ["tenant_id", "actor_id", "order_id", "customer_id", "operation"]) {
  requireText(diagnosticContext, `${field}: &'static str`, `${paths.source}: bounded ${field}`);
}
for (const forbidden of ["Uuid", "Option<", "String"]) {
  forbidText(diagnosticContext, forbidden, `${paths.source}: mutation context storage`);
}

const contextConversion = blockBetween(
  source,
  "impl From<&AdminOrderErrorContext> for AdminOrderMutationDiagnosticContext {",
  "struct AdminOrderMutationDiagnosticError;",
  "mutation context conversion",
);
for (const marker of [
  "tenant_id: uuid_shape(context.tenant_id)",
  "actor_id: uuid_shape(context.actor_id)",
  "order_id: optional_uuid_shape(context.order_id)",
  "customer_id: optional_uuid_shape(context.customer_id)",
  "operation: context.operation",
]) requireText(contextConversion, marker, `${paths.source}: mutation context conversion`);

const diagnosticError = blockBetween(
  source,
  "struct AdminOrderMutationDiagnosticError;",
  "fn uuid_shape(",
  "bounded mutation error",
);
for (const marker of [
  "impl std::fmt::Debug for AdminOrderMutationDiagnosticError",
  'formatter.write_str("redacted")',
]) requireText(diagnosticError, marker, `${paths.source}: bounded mutation error`);
for (const forbidden of ["OrderError", "message:", "source:", "String"]) {
  forbidText(diagnosticError, forbidden, `${paths.source}: mutation error payload`);
}

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
  "fn text_presence_shape(",
  "optional UUID shape",
);
for (const marker of [
  'None => "absent"',
  'Some(value) if value.is_nil() => "present_nil"',
  'Some(_) => "present_non_nil"',
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

const policy = blockBetween(
  source,
  "fn admin_order_error_policy(",
  "fn map_admin_order_error(",
  "order mutation policy",
);
for (const marker of [
  "OrderError::Validation(_)",
  "OrderError::OrderNotFound(_)",
  "OrderError::OrderReturnNotFound(_)",
  "OrderError::OrderChangeNotFound(_)",
  "OrderError::InvalidTransition { .. }",
  "OrderError::Database(_)",
  "OrderError::Core(_)",
  '"commerce_admin_order_invalid"',
  '"commerce_admin_not_found"',
  '"commerce_admin_order_state_conflict"',
  '"commerce_admin_order_storage_unavailable"',
  '"commerce_admin_order_failed"',
]) requireText(policy, marker, `${paths.source}: preserved mutation policy`);

const mapper = blockBetween(
  source,
  "fn map_admin_order_error(",
  "/// Show admin ecommerce order",
  "admin order mutation mapper",
);
requireOrder(
  mapper,
  [
    "if let OrderError::OrderNotFound(id) = &error",
    "context.order_id = Some(*id);",
    "let (status, code, message, error_kind) = admin_order_error_policy(&error);",
    "let context = AdminOrderMutationDiagnosticContext::from(&context);",
    "let error = AdminOrderMutationDiagnosticError;",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: identity policy and shadowing order`,
);
for (const marker of [
  "error = ?error",
  "owner = ADMIN_ORDER_OWNER",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "order_id = ?context.order_id",
  "customer_id = ?context.customer_id",
  "operation = %context.operation",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_ORDER_BOUNDARY",
  '"commerce admin order operation failed"',
]) requireText(mapper, marker, `${paths.source}: bounded mutation log site`);
for (const forbidden of [
  "error.to_string()",
  "error.message",
  "format!(",
  "tracing::error!(\n        error = ?error,\n        owner = ADMIN_ORDER_OWNER,\n        tenant_id = %context.tenant_id",
]) {
  if (forbidden.startsWith("tracing::error!")) continue;
  forbidText(mapper, forbidden, `${paths.source}: raw mutation payload`);
}

requireCount(source, "map_admin_order_error(", 5, "one mutation mapper and four callsites");
requireCount(source, "map_admin_order_port_error(", 3, "one read mapper and two callsites");
requireCount(source, "&[Permission::ORDERS_UPDATE]", 4, "four mutation permissions");
for (const marker of [
  '"mark_order_paid"',
  '"ship_order"',
  '"deliver_order"',
  '"cancel_order"',
  ".mark_paid(",
  ".ship_order(",
  ".deliver_order(tenant.id, auth.user_id, id, input.delivered_signature)",
  ".cancel_order(tenant.id, auth.user_id, id, input.reason)",
]) requireText(source, marker, `${paths.source}: preserved mutation route`);

for (const marker of [
  "fn map_admin_order_port_error(",
  "AdminOrderReadDiagnosticContext::from(&context)",
  "AdminOrderReadPortDiagnosticContext::from(port_context)",
  "fn map_order_detail_payment_error(",
  "fn map_order_detail_fulfillment_error(",
  '"commerce admin order detail payment lookup failed"',
  '"commerce admin order detail fulfillment lookup failed"',
]) requireText(source, marker, `${paths.source}: unchanged neighboring mapper`);

for (const marker of [
  "if let OrderError::OrderNotFound(id) = &error",
  "context.order_id = Some(*id);",
  "error = ?error",
  "HttpError::new(status, code, message)",
]) requireText(broadVerifier, marker, `${paths.broadVerifier}: compatibility marker`);

if (
  evidence.status !==
  "commerce_admin_order_mutation_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_order_error_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_order_uuid_logged: false,
  raw_customer_uuid_logged: false,
  redacted_error_debug_logged: true,
  required_uuid_shapes_logged: true,
  optional_uuid_shapes_logged: true,
  operation_preserved: true,
  order_not_found_identity_adoption_preserved: true,
  typed_policy_selection_precedes_shadowing: true,
  order_error_policy_preserved: true,
  http_envelopes_preserved: true,
  four_mutation_callsites_preserved: true,
  orders_update_permission_preserved: true,
  read_mapper_unchanged: true,
  detail_payment_mapper_unchanged: true,
  detail_fulfillment_mapper_unchanged: true,
  existing_broad_verifier_markers_preserved: true,
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
  "order-not-found identity adoption and HTTP policy selection",
  "Debug output is always `redacted`",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin order mutation diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin order mutation diagnostics are bounded while identity adoption, four owner calls, permissions, and HTTP policies remain unchanged; execution validation remains open",
);
