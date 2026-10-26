#!/usr/bin/env node

// Admin order-change owner error context.
//
// `POST /admin/order-changes/{id}/apply` (`controllers/admin/changes.rs`) is the only admin
// order-change route left. It builds two port contexts — a read context and an idempotent command
// context — and hands them to `OrderChangeOrchestrationService::from_order_ports`, so the failure it
// owns at the boundary is a `PortError`: kind, bounded code, retryability. Each kind maps onto one
// static Commerce envelope and the diagnostic line records shape only.
//
// The verifier used to describe the retired compatibility surface: a typed `OrderError` cause with
// `error = ?error`, `ADMIN_ORDER_CHANGE_OWNER`, `admin_order_change_read_context` /
// `admin_order_change_apply_context` plus `.create_order_change(...)`, `.list_order_changes(...)`,
// `.get_order_change(...)` and `.cancel_order_change(...)` running against `OrderService` in-process,
// and three private policy copies. The in-process paths are gone, the policies are shared from
// `controllers/admin/mod.rs`, and the guard is re-anchored on the owner-port contract.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const controller = read("crates/modules/rustok-commerce/src/controllers/admin/changes.rs");
const adminMod = read("crates/modules/rustok-commerce/src/controllers/admin/mod.rs");
const portErrors = read("crates/libs/rustok-api/src/ports.rs");
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const requirePattern = (content, pattern, label) => {
  if (!pattern.test(content)) failures.push(`${label}: missing ${pattern}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, pattern, expected, label) => {
  const actual = content.match(pattern)?.length ?? 0;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex);
};
const normalize = (content) => content.replace(/\s+/g, " ");
const requireTextNormalized = (content, value, label) => {
  if (!normalize(content).includes(normalize(value))) failures.push(`${label}: missing ${value}`);
};
const requireOrdered = (content, values, label) => {
  let cursor = -1;
  for (const value of values) {
    const index = content.indexOf(value, cursor + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    cursor = index;
  }
};
const enumVariants = (content, name, label) => {
  const marker = `pub enum ${name} {`;
  const start = content.indexOf(marker);
  if (start < 0) {
    failures.push(`${label}: missing enum ${name}`);
    return [];
  }
  const body = content.slice(start + marker.length);
  const end = body.indexOf("\n}");
  if (end < 0) {
    failures.push(`${label}: unterminated enum ${name}`);
    return [];
  }
  return [...body.slice(0, end).matchAll(/^ {4}([A-Z][A-Za-z0-9]*)\b/gm)].map(
    (match) => match[1],
  );
};

const imports = between(controller, "use super::{", "use crate::services::", "order-change imports");
const constants = between(
  controller,
  "const ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER:",
  "type AdminOrderChangeHttpPolicy",
  "order-change constants",
);
const policyAlias = between(
  controller,
  "type AdminOrderChangeHttpPolicy",
  "fn log_admin_order_change_entry(",
  "order-change policy alias",
);
const entryLogger = between(
  controller,
  "fn log_admin_order_change_entry(",
  "fn require_idempotency_key(",
  "order-change entry logger",
);
const readContext = between(
  controller,
  "fn admin_order_change_read_context(",
  "fn admin_order_change_apply_context(",
  "order-change read context",
);
const applyContext = between(
  controller,
  "fn admin_order_change_apply_context(",
  "fn admin_order_change_port_error_policy(",
  "order-change apply context",
);
const portPolicy = between(
  controller,
  "fn admin_order_change_port_error_policy(",
  "fn optional_uuid_shape(",
  "order-change port policy",
);
const portMapper = between(
  controller,
  "fn map_admin_order_change_port_error(",
  "fn map_admin_order_change_orchestration_error(",
  "order-change port mapper",
);
const applyRoute = from(controller, "pub async fn apply_order_change(", "apply order change route");

// --- Wiring ------------------------------------------------------------------------------------

for (const [value, label] of [
  ["AdminErrorPolicy,", "shared policy type import"],
  ["order_error_policy,", "shared order policy import"],
  ["payment_error_policy,", "shared payment policy import"],
  ["reserved_refund_error_policy,", "shared reserved refund policy import"],
]) requireText(imports, value, label);

requireText(
  policyAlias,
  "type AdminOrderChangeHttpPolicy = AdminErrorPolicy;",
  "order-change policy alias reuses the shared admin policy shape",
);

requireText(
  constants,
  'const ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_http";',
  "order-change boundary",
);

for (const forbidden of [
  "fn admin_order_change_payment_error_policy(",
  "fn admin_order_change_reserved_refund_error_policy(",
  "fn admin_order_change_order_error_policy(",
]) forbidText(controller, forbidden, "duplicated per-owner policy in the order-change controller");

// --- Bounded entry line ------------------------------------------------------------------------

for (const [value, label] of [
  ["operation: &'static str,", "entry operation"],
  ["order_change_id: Uuid,", "entry order-change identity"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "entry actor shape"],
  ["order_change_id_non_nil = !order_change_id.is_nil(),", "entry order-change shape"],
  ["boundary = ADMIN_ORDER_CHANGE_BOUNDARY,", "entry boundary"],
  ['"commerce admin order change route entered with bounded diagnostics"', "entry message"],
]) requireText(entryLogger, value, label);

requireCount(controller, /log_admin_order_change_entry\(/g, 2, "one entry helper and one callsite");
requireCount(controller, /tracing::debug!\(/g, 1, "one debug line");
requireCount(controller, /tracing::error!\(/g, 2, "two owner mapper error lines");
requireCount(controller, /tracing::warn!\(/g, 0, "no warn line");

forbidRawContextLogging(failures, controller, "admin order-change controller");
for (const forbidden of [
  "error = ?error",
  "tenant_id = %tenant.id",
  "actor_id = %auth.user_id",
  "order_change_id = %id",
  ".create_order_change(",
  ".list_order_changes(",
  ".get_order_change(tenant.id, id)",
  ".cancel_order_change(tenant.id, id, input)",
  "PaymentService::new(",
]) forbidText(controller, forbidden, "retired in-process order-change path");

// --- Port contexts -----------------------------------------------------------------------------

for (const [value, label] of [
  ["PortContext::new(", "read context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "read actor projection"],
  ["request_context.locale.as_str(),", "read locale projection"],
  ['format!("commerce-admin-order-change:read:{resource_id}")', "read correlation key"],
  [".with_deadline(std::time::Duration::from_secs(2))", "read deadline"],
  ["context = context.with_channel(channel);", "read channel forwarding"],
]) requireTextNormalized(readContext, value, `read context: ${label}`);

for (const [value, label] of [
  ["PortContext::new(", "apply context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "apply actor projection"],
  ['format!("commerce-admin-order-change:apply:{resource_id}")', "apply correlation key"],
  [".with_idempotency_key(idempotency_key)", "apply idempotency key"],
  [".with_deadline(std::time::Duration::from_secs(2))", "apply deadline"],
  ["context = context.with_channel(channel);", "apply channel forwarding"],
]) requireTextNormalized(applyContext, value, `apply context: ${label}`);

// --- Port policy and mapper --------------------------------------------------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const variant of portErrorKinds) {
  requirePattern(
    portPolicy,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `order-change port policy handles PortErrorKind::${variant}`,
  );
}
forbidText(portPolicy, "        _ =>", "wildcard in the order-change port policy");

for (const [value, label] of [
  ["StatusCode::BAD_REQUEST", "validation status"],
  ['"commerce_admin_order_invalid"', "validation code"],
  ['"Order request is invalid"', "validation message"],
  ["StatusCode::NOT_FOUND", "not-found status"],
  ['"commerce_admin_not_found"', "not-found code"],
  ['"Commerce resource not found"', "not-found message"],
  ["StatusCode::CONFLICT", "conflict status"],
  ['"commerce_admin_order_state_conflict"', "conflict code"],
  ['"Order operation conflicts with the current state"', "conflict message"],
  ["StatusCode::FORBIDDEN", "forbidden status"],
  ['"commerce_permission_denied"', "forbidden code"],
  ["StatusCode::SERVICE_UNAVAILABLE", "storage status"],
  ['"commerce_admin_order_storage_unavailable"', "storage code"],
  ['"Order storage is temporarily unavailable"', "storage message"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
  ['"commerce_admin_order_failed"', "fail-closed code"],
  ['"Order operation could not be completed safely"', "fail-closed message"],
]) requireText(portPolicy, value, `${label}`);

for (const [value, label] of [
  ['owner = "rustok_order",', "owner label"],
  ["owner_operation,", "owner operation"],
  ['consumer_operation = "apply_order_change",', "consumer operation"],
  ["correlation_id = %context.correlation_id,", "correlation join key"],
  ["tenant_id_non_empty = !context.tenant_id.is_empty(),", "tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape"],
  ["order_change_id_non_nil = !order_change_id.is_nil(),", "order-change shape"],
  ["owner_error_kind = ?error.kind,", "typed owner kind"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
  ["retryable = error.retryable,", "retryability"],
  ["public_code = code,", "public code"],
  ["status = %status,", "status"],
  ["boundary = ADMIN_ORDER_CHANGE_BOUNDARY,", "boundary"],
  ['"commerce admin order-change owner port failed with bounded diagnostics"', "mapper message"],
  ["HttpError::new(status, code, message)", "single static envelope constructor"],
]) requireText(portMapper, value, `order-change port mapper: ${label}`);

for (const forbidden of ["%error", "error.message", ".to_string()", "format!("]) {
  forbidText(portMapper, forbidden, "raw order-change port payload");
}

// --- Apply route -------------------------------------------------------------------------------

for (const [value, label] of [
  ["ensure_permissions(", "permission gate"],
  ["[Permission::ORDERS_UPDATE]", "update permission"],
  ['"Permission denied: orders:update required"', "update permission message"],
  ['"commerce_invalid_tenant"', "tenant guard"],
  ['"commerce_invalid_id"', "id guard"],
  ['log_admin_order_change_entry("apply_order_change", tenant.id, auth.user_id, id);',
   "bounded entry line"],
  ["require_idempotency_key(&headers)?", "idempotency key"],
  ["admin_order_change_read_context(&tenant, &auth, &request_context, id)", "read context"],
  [
    "admin_order_change_apply_context(&tenant, &auth, &request_context, id, idempotency_key)",
    "apply context",
  ],
  ["OrderChangeOrchestrationService::from_order_ports(", "owner-port orchestration"],
  ["runtime.order_read_port(),", "host-selected Order read port"],
  ["runtime.order_post_order_command_port(),", "host-selected Order command port"],
  [".with_payment_provider_registry(runtime.payment_provider_registry())",
   "payment provider registry forwarding"],
  [".apply_order_change_with_owner_ports(", "owner-port apply call"],
  ["tenant.id,", "apply tenant identity"],
  ["read_context.clone(),", "read context handoff"],
  ["command_context.clone(),", "command context handoff"],
  ["input.difference_refund,", "difference refund forwarding"],
  ["input.metadata,", "metadata forwarding"],
  [".map_err(|error| {", "typed mapping closure"],
  ["map_admin_order_change_apply_error(", "apply mapper handoff"],
  ["AdminOrderChangeOrchestrationErrorContext::new(", "apply error context"],
  ['"apply_order_change",', "apply operation label"],
  ["Ok(Json(result))", "apply response"],
]) requireTextNormalized(applyRoute, value, `apply order change route: ${label}`);

requireOrdered(
  applyRoute,
  [
    "[Permission::ORDERS_UPDATE]",
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_change_entry(",
    "require_idempotency_key(&headers)?",
    ".apply_order_change_with_owner_ports(",
    ".map_err(|error| {",
  ],
  "apply order change route order",
);

requireText(
  adminMod,
  "pub(crate) fn map_post_order_orchestration_error(",
  "shared post-order orchestration mapper",
);

if (failures.length > 0) {
  console.error("Commerce admin order-change owner error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order-change apply keeps owner-port contexts, static PortError envelopes, and bounded diagnostics",
);
