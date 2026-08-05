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
    "crates/rustok-commerce/contracts/evidence/admin-order-read-diagnostic-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-order-read-diagnostic-safety.md",
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
  "struct AdminOrderReadDiagnosticContext {",
  "impl From<&AdminOrderErrorContext> for AdminOrderReadDiagnosticContext {",
  "bounded order diagnostic context",
);
for (const field of ["tenant_id", "actor_id", "order_id", "customer_id", "operation"]) {
  requireText(diagnosticContext, `${field}: &'static str`, `${paths.source}: bounded ${field}`);
}
for (const forbidden of ["Uuid", "Option<", "String"]) {
  forbidText(diagnosticContext, forbidden, `${paths.source}: bounded order context storage`);
}

const contextConversion = blockBetween(
  source,
  "impl From<&AdminOrderErrorContext> for AdminOrderReadDiagnosticContext {",
  "struct AdminOrderReadPortDiagnosticContext {",
  "order diagnostic conversion",
);
for (const marker of [
  "tenant_id: uuid_shape(context.tenant_id)",
  "actor_id: uuid_shape(context.actor_id)",
  "order_id: optional_uuid_shape(context.order_id)",
  "customer_id: optional_uuid_shape(context.customer_id)",
  "operation: context.operation",
]) requireText(contextConversion, marker, `${paths.source}: order diagnostic conversion`);

const portDiagnosticContext = blockBetween(
  source,
  "struct AdminOrderReadPortDiagnosticContext {",
  "struct AdminOrderReadPortDiagnosticError<'a> {",
  "bounded port diagnostic context",
);
for (const marker of [
  "correlation_id: &'static str",
  "actor: &'static str",
  "channel: &'static str",
  "locale: usize",
  "deadline_ms: Option<u64>",
  "correlation_id: text_presence_shape(context.correlation_id.as_str())",
  "actor: text_presence_shape(context.actor.id.as_str())",
  "channel: optional_text_presence_shape(context.channel.as_deref())",
  "locale: context.locale.len()",
  "deadline_ms: context.deadline_ms",
]) requireText(portDiagnosticContext, marker, `${paths.source}: bounded port context`);
for (const forbidden of ["correlation_id: String", "actor: PortActor", "channel: Option<String>", "locale: String"]) {
  forbidText(portDiagnosticContext, forbidden, `${paths.source}: raw port context storage`);
}

const diagnosticError = blockBetween(
  source,
  "struct AdminOrderReadPortDiagnosticError<'a> {",
  "fn uuid_shape(",
  "bounded diagnostic error",
);
for (const marker of [
  "code: &'a str",
  "retryable: bool",
  "impl std::fmt::Debug for AdminOrderReadPortDiagnosticError<'_>",
  'formatter.write_str("redacted")',
]) requireText(diagnosticError, marker, `${paths.source}: bounded diagnostic error`);
for (const forbidden of ["message:", "kind:", "source:"]) {
  forbidText(diagnosticError, forbidden, `${paths.source}: diagnostic error payload`);
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

const textShapes = blockBetween(
  source,
  "fn text_presence_shape(",
  "fn admin_order_read_port_context(",
  "text presence shapes",
);
for (const marker of [
  '"empty"',
  '"present_non_empty"',
  'None => "absent"',
  'Some("") => "present_empty"',
]) requireText(textShapes, marker, `${paths.source}: text presence shape`);

const mapper = blockBetween(
  source,
  "fn map_admin_order_port_error(",
  "fn admin_order_error_policy(",
  "admin order read mapper",
);
for (const marker of [
  "PortErrorKind::Validation",
  "PortErrorKind::NotFound",
  "PortErrorKind::Conflict",
  "PortErrorKind::Forbidden",
  "PortErrorKind::Unavailable | PortErrorKind::Timeout",
  "PortErrorKind::InvariantViolation",
  '"commerce_admin_order_invalid"',
  '"commerce_admin_not_found"',
  '"commerce_admin_order_state_conflict"',
  '"commerce_permission_denied"',
  '"commerce_admin_order_storage_unavailable"',
  '"commerce_admin_order_failed"',
]) requireText(mapper, marker, `${paths.source}: preserved read policy`);

requireOrder(
  mapper,
  [
    "let (status, code, message, error_kind) = match &error.kind",
    "let context = AdminOrderReadDiagnosticContext::from(&context);",
    "let port_context = AdminOrderReadPortDiagnosticContext::from(port_context);",
    "let error = AdminOrderReadPortDiagnosticError {",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: typed policy then bounded shadowing`,
);

for (const marker of [
  "error = ?error",
  "owner = ADMIN_ORDER_OWNER",
  "owner_operation,",
  "correlation_id = %port_context.correlation_id",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "order_id = ?context.order_id",
  "customer_id = ?context.customer_id",
  "operation = %context.operation",
  "actor = ?port_context.actor",
  "channel = ?port_context.channel",
  "locale = %port_context.locale",
  "deadline_ms = ?port_context.deadline_ms",
  "internal_code = %error.code",
  "retryable = error.retryable",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_ORDER_BOUNDARY",
  '"commerce admin order owner read failed"',
]) requireText(mapper, marker, `${paths.source}: retained bounded log-site marker`);
for (const forbidden of ["error.message", "error.to_string()", "format!(", "context: &AdminOrderErrorContext"]) {
  forbidText(mapper, forbidden, `${paths.source}: raw mapper payload`);
}

requireCount(source, "map_admin_order_port_error(", 3, "one read mapper and two callsites");
requireCount(source, "map_admin_order_error(", 5, "one mutation mapper and four callsites");
for (const marker of [
  ".list_order_projections(",
  ".read_order_projection(",
  '"list_order_projections"',
  '"read_order_projection"',
  "tenant_default_locale: Some(tenant.default_locale.clone())",
  "per_page: pagination.limit()",
  "&[Permission::ORDERS_LIST]",
  "&[Permission::ORDERS_READ]",
]) requireText(source, marker, `${paths.source}: preserved read route`);

const mutationMapper = blockBetween(
  source,
  "fn admin_order_error_policy(",
  "/// Show admin ecommerce order",
  "mutation mapper",
);
for (const marker of [
  "fn map_admin_order_error(",
  "if let OrderError::OrderNotFound(id) = &error",
  "error = ?error",
  '"commerce admin order operation failed"',
]) requireText(mutationMapper, marker, `${paths.source}: unchanged mutation mapper`);

for (const marker of [
  "fn map_order_detail_payment_error(",
  "fn map_order_detail_fulfillment_error(",
  '"commerce admin order detail payment lookup failed"',
  '"commerce admin order detail fulfillment lookup failed"',
]) requireText(source, marker, `${paths.source}: unchanged detail mapper`);

for (const marker of [
  "correlation_id = %port_context.correlation_id",
  "internal_code = %error.code",
  "retryable = error.retryable",
  "HttpError::new(status, code, message)",
]) requireText(broadVerifier, marker, `${paths.broadVerifier}: compatibility marker`);

if (
  evidence.status !==
  "commerce_admin_order_read_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_port_error_logged: false,
  raw_correlation_id_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_order_uuid_logged: false,
  raw_customer_uuid_logged: false,
  raw_port_actor_logged: false,
  raw_channel_logged: false,
  raw_locale_logged: false,
  redacted_error_debug_logged: true,
  stable_internal_code_logged: true,
  retryability_logged: true,
  required_uuid_shapes_logged: true,
  optional_uuid_shapes_logged: true,
  existing_broad_verifier_markers_preserved: true,
  port_error_kind_policy_preserved: true,
  two_admin_order_read_callsites_preserved: true,
  mutation_mapper_unchanged: true,
  detail_payment_mapper_unchanged: true,
  detail_fulfillment_mapper_unchanged: true,
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
  "shadowed before `tracing::error!`",
  "a redacted Debug representation for the error",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin order read diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin order read diagnostics are bounded while the two owner-port routes and HTTP policies remain unchanged; execution validation remains open",
);
