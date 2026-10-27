#!/usr/bin/env node

// Admin order-return owner error context.
//
// The four admin order-return routes speak to the Order owner through host-selected ports:
//
//   POST /admin/orders/{id}/returns            -> create_order_return   (post_order_commands.rs)
//   POST /admin/returns/{id}/cancel            -> cancel_order_return   (post_order_commands.rs)
//   GET  /admin/returns                        -> list_order_returns    (post_order_reads.rs)
//   GET  /admin/returns/{id}                   -> show_order_return     (post_order_reads.rs)
//
// A failure crosses the boundary as `PortError`, so the typed cause the controller owns is the port
// failure — kind, bounded code, retryability — never owner text and never a raw identity value. Each
// failure is projected onto a static Commerce envelope, and the diagnostic line records shape.
//
// The verifier used to describe the retired direct-service implementation: `create_order_return` et
// al. were handlers in `admin/returns.rs` calling `OrderService` directly with a typed `OrderError`
// cause, an `AdminOrderReturnErrorContext` and `map_admin_order_return_error`. Those handlers moved to
// `post_order_commands.rs` / `post_order_reads.rs` behind `order_post_order_command_port()` and
// `order_read_port()`, so the guard is re-anchored on the port contract and on the shared admin
// policies in `controllers/admin/mod.rs`, which the return-orchestration guard covers.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const commands = read(
  "crates/modules/rustok-commerce/src/controllers/admin/post_order_commands.rs",
);
const reads = read("crates/modules/rustok-commerce/src/controllers/admin/post_order_reads.rs");
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

const commandConstants = between(
  commands,
  "const ADMIN_POST_ORDER_OWNER:",
  "fn log_admin_post_order_command_entry(",
  "post-order command constants",
);
const commandEntryLogger = between(
  commands,
  "fn log_admin_post_order_command_entry(",
  "fn require_idempotency_key(",
  "post-order command entry logger",
);
const commandContext = between(
  commands,
  "fn admin_post_order_command_context(",
  "fn map_admin_post_order_port_error(",
  "post-order command context builder",
);
const commandMapper = between(
  commands,
  "fn map_admin_post_order_port_error(",
  "#[utoipa::path(",
  "post-order command port mapper",
);
const createReturnRoute = between(
  commands,
  "pub async fn create_order_return(",
  "pub async fn cancel_order_return(",
  "create order return route",
);
const cancelReturnRoute = from(commands, "pub async fn cancel_order_return(", "cancel order return route");

const readConstants = between(
  reads,
  "const ADMIN_POST_ORDER_OWNER:",
  "fn log_admin_post_order_read_entry(",
  "post-order read constants",
);
const readEntryLogger = between(
  reads,
  "fn log_admin_post_order_read_entry(",
  "fn admin_post_order_read_context(",
  "post-order read entry logger",
);
const readContext = between(
  reads,
  "fn admin_post_order_read_context(",
  "fn uuid_shape(",
  "post-order read context builder",
);
const readMapper = between(
  reads,
  "fn map_admin_post_order_port_error(",
  "#[utoipa::path(",
  "post-order read port mapper",
);
const listReturnsRoute = between(
  reads,
  "pub async fn list_order_returns(",
  "pub async fn show_order_return(",
  "list order returns route",
);
const showReturnRoute = between(
  reads,
  "pub async fn show_order_return(",
  "pub async fn list_order_changes(",
  "show order return route",
);

// --- Boundary constants and bounded entry lines -------------------------------------------------

for (const [value, label] of [
  ['const ADMIN_POST_ORDER_OWNER: &str = "rustok_order.post_order_command";', "command owner"],
  [
    'const ADMIN_POST_ORDER_BOUNDARY: &str = "commerce_admin_post_order_command_http";',
    "command boundary",
  ],
]) requireText(commandConstants, value, `post_order_commands.rs: ${label}`);

for (const [value, label] of [
  ['const ADMIN_POST_ORDER_OWNER: &str = "rustok_order.admin_post_order_reads";', "read owner"],
  [
    'const ADMIN_POST_ORDER_BOUNDARY: &str = "commerce_admin_post_order_http";',
    "read boundary",
  ],
]) requireText(readConstants, value, `post_order_reads.rs: ${label}`);

for (const [value, label] of [
  ["operation: &'static str,", "entry operation"],
  ["resource_kind: &'static str,", "entry resource kind"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "entry actor shape"],
  ["resource_id_non_nil = !resource_id.is_nil(),", "entry resource shape"],
  ["boundary = ADMIN_POST_ORDER_BOUNDARY,", "entry boundary"],
  ['"commerce admin post-order command entered with bounded diagnostics"', "entry message"],
]) requireText(commandEntryLogger, value, label);

for (const [value, label] of [
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "entry actor shape"],
  ["resource_id_present = resource_id.is_some(),", "entry resource presence"],
  [
    "resource_id_non_nil = resource_id.is_some_and(|value| !value.is_nil()),",
    "entry resource shape",
  ],
  ["boundary = ADMIN_POST_ORDER_BOUNDARY,", "entry boundary"],
  ['"commerce admin post-order read entered with bounded diagnostics"', "entry message"],
]) requireText(readEntryLogger, value, `post_order_reads.rs: ${label}`);

requireCount(
  commands,
  /log_admin_post_order_command_entry\(\n        "/g,
  4,
  "post_order_commands.rs: four bounded entry lines",
);
requireCount(commands, /tracing::debug!\(/g, 1, "post_order_commands.rs: one debug line");
requireCount(commands, /tracing::error!\(/g, 1, "post_order_commands.rs: one error line");
requireCount(commands, /tracing::warn!\(/g, 0, "post_order_commands.rs: no warn line");
requireCount(reads, /tracing::debug!\(/g, 1, "post_order_reads.rs: one debug line");
requireCount(reads, /tracing::error!\(/g, 1, "post_order_reads.rs: one error line");

forbidRawContextLogging(failures, commands, "admin post-order command controller");
forbidRawContextLogging(failures, reads, "admin post-order read controller");
for (const [content, label] of [
  [commands, "post_order_commands.rs"],
  [reads, "post_order_reads.rs"],
]) {
  for (const forbidden of ["= %tenant_id", "= %actor_id", "error = ?error", "error = ?self"]) {
    forbidText(content, forbidden, `${label}: raw context directive`);
  }
}

// The policy set is shared; neither controller may regrow a private copy.
for (const [content, label] of [
  [commands, "post_order_commands.rs"],
  [reads, "post_order_reads.rs"],
]) {
  for (const forbidden of [
    "fn admin_order_error_policy(",
    "fn admin_payment_error_policy(",
    "fn admin_reserved_refund_error_policy(",
  ]) {
    forbidText(content, forbidden, `${label}: duplicated per-owner policy`);
  }
}

// --- Port context builders carry identity into the port surface, shape out of the log line --------

for (const [value, label] of [
  ["PortContext::new(", "port context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "actor projection"],
  ["tenant.id.to_string()", "tenant projection"],
  ["format!(\"commerce-admin-post-order:{operation}:{resource_id}\")", "correlation key"],
  [".with_idempotency_key(idempotency_key)", "idempotency key forwarding"],
  [".with_deadline(std::time::Duration::from_secs(2))", "port deadline"],
]) requireText(commandContext, value, label);

for (const [value, label] of [
  ["PortContext::new(", "port context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "actor projection"],
  ["format!(\"commerce-admin-post-order:{operation}:{resource_id}\")", "correlation key"],
  [".with_deadline(std::time::Duration::from_secs(2))", "port deadline"],
]) requireText(readContext, value, label);

// --- Port kind policies: every PortErrorKind has one static envelope -----------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}

for (const [mapper, label] of [
  [commandMapper, "post_order_commands.rs port mapper"],
  [readMapper, "post_order_reads.rs port mapper"],
]) {
  for (const variant of portErrorKinds) {
    requirePattern(
      mapper,
      new RegExp(`PortErrorKind::${variant}\\b`),
      `${label} handles PortErrorKind::${variant}`,
    );
  }
  for (const [value, envelope] of [
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
    ['"Permission denied"', "forbidden message"],
    ["StatusCode::SERVICE_UNAVAILABLE", "unavailable status"],
    ['"commerce_admin_order_storage_unavailable"', "storage code"],
    ['"Order storage is temporarily unavailable"', "storage message"],
    ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
    ['"commerce_admin_order_failed"', "fail-closed code"],
    ['"Order operation could not be completed safely"', "fail-closed message"],
    ["HttpError::new(status, code, message)", "single static envelope constructor"],
  ]) requireText(mapper, value, `${label}: ${envelope}`);
}

for (const [value, label] of [
  ["owner = ADMIN_POST_ORDER_OWNER,", "command owner field"],
  ["owner_code_length = error.code.chars().count(),", "command bounded owner code"],
  ["retryable = error.retryable,", "command retryability"],
  ["owner_error_kind = ?error.kind,", "command typed owner kind"],
  ["correlation_id = %context.correlation_id,", "command correlation join key"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "command tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "command actor shape"],
  ["resource_id_non_nil = !resource_id.is_nil(),", "command resource shape"],
  ["public_code = code,", "command public code"],
  ["status = %status,", "command status"],
  ["boundary = ADMIN_POST_ORDER_BOUNDARY,", "command boundary"],
]) requireText(commandMapper, value, `post_order_commands.rs: ${label}`);

for (const [value, label] of [
  ["owner = ADMIN_POST_ORDER_OWNER,", "read owner field"],
  ["owner_operation,", "read owner operation"],
  ["consumer_operation,", "read consumer operation"],
  ["correlation_id_present,", "read correlation presence"],
  ["correlation_id_length,", "read correlation length"],
  ["tenant_id_shape,", "read tenant shape"],
  ["actor_id_shape,", "read actor shape"],
  ["return_id_shape,", "read return shape"],
  ["change_id_shape,", "read change shape"],
  ["order_id_shape,", "read order shape"],
  ["channel_present,", "read channel presence"],
  ["channel_length,", "read channel length"],
  ["locale_length,", "read locale length"],
  ["deadline_ms = ?port_context.deadline_ms,", "read deadline"],
  ["owner_code_length,", "read bounded owner code"],
  ["retryable,", "read retryability"],
  ["boundary = ADMIN_POST_ORDER_BOUNDARY,", "read boundary"],
]) requireText(readMapper, value, `post_order_reads.rs: ${label}`);

for (const [value, label] of [
  ["let tenant_id_shape = uuid_text_shape(port_context.tenant_id.as_str());", "tenant text shape"],
  ["let channel_present = port_context.channel.is_some();", "channel presence"],
  ["let error_kind = match error.kind {", "typed error kind projection"],
]) requireText(readMapper, value, `post_order_reads.rs: ${label}`);

// --- Route contracts ----------------------------------------------------------------------------

for (const [route, label] of [
  [createReturnRoute, "create order return route"],
  [cancelReturnRoute, "cancel order return route"],
]) {
  requireText(route, "[Permission::ORDERS_UPDATE]", `${label} permission`);
  requireText(route, '"Permission denied: orders:update required"', `${label} permission message`);
  requireText(route, '"invalid_tenant_id"', `${label} tenant guard`);
  requireText(route, "require_idempotency_key(&headers)?", `${label} idempotency key`);
  requireText(route, "order_post_order_command_port()", `${label} host-selected owner port`);
  requireText(route, "context.clone()", `${label} port context handoff`);
  requireText(route, ".map_err(|error| {", `${label} typed mapping closure`);
  requireText(route, "map_admin_post_order_port_error(", `${label} mapper handoff`);
  requireText(route, "log_admin_post_order_command_entry(", `${label} bounded entry line`);
  requireOrdered(
    route,
    ["if tenant.id.is_nil()", "if id.is_nil()", "log_admin_post_order_command_entry("],
    `${label} guards before entry line`,
  );
  requireOrdered(
    route,
    ["log_admin_post_order_command_entry(", "require_idempotency_key(&headers)?", ".map_err(|error| {"],
    `${label} entry line before the owner call`,
  );
}

for (const [value, label] of [
  ["pub async fn create_order_return(", "create return handler"],
  ['"invalid_order_id"', "create return order guard"],
  ["OwnerCreateOrderReturnRequest {", "typed create-return request"],
  ["order_id: id,", "create return order identity"],
  [".create_return(", "create return owner command"],
  ['"create_return",', "create return operation label"],
  ["Ok((StatusCode::CREATED, Json(created)))", "create return created response"],
]) requireText(createReturnRoute, value, `create order return route: ${label}`);

for (const [value, label] of [
  ["pub async fn cancel_order_return(", "cancel return handler"],
  ['"invalid_order_return_id"', "cancel return id guard"],
  ["OwnerCancelOrderReturnRequest {", "typed cancel-return request"],
  ["return_id: id,", "cancel return identity"],
  [".cancel_return(", "cancel return owner command"],
  ['"cancel_return",', "cancel return operation label"],
  ["Ok(Json(item))", "cancel return response"],
]) requireText(cancelReturnRoute, value, `cancel order return route: ${label}`);

for (const [route, label] of [
  [listReturnsRoute, "list order returns route"],
  [showReturnRoute, "show order return route"],
]) {
  requireText(route, "[Permission::ORDERS_READ]", `${label} permission`);
  requireText(route, '"Permission denied: orders:read required"', `${label} permission message`);
  requireText(route, '"invalid_tenant_id"', `${label} tenant guard`);
  requireText(route, "order_read_port()", `${label} host-selected owner port`);
  requireText(route, "context.clone()", `${label} port context handoff`);
  requireText(route, ".map_err(|error| {", `${label} typed mapping closure`);
  requireText(route, "map_admin_post_order_port_error(", `${label} mapper handoff`);
  requireText(route, "log_admin_post_order_read_entry(", `${label} bounded entry line`);
  requireOrdered(
    route,
    ["if tenant.id.is_nil()", "log_admin_post_order_read_entry("],
    `${label} tenant guard before entry line`,
  );
}

for (const [value, label] of [
  ["ListOrderReturnProjectionsRequest {", "typed list-return request"],
  ["page: pagination.page,", "list page forwarding"],
  ["per_page: pagination.limit(),", "list page-size forwarding"],
  ["order_id,", "list order filter forwarding"],
  ["status: params.status,", "list status filter forwarding"],
  [".list_order_return_projections(", "list owner read"],
  [
    '"list_order_return_projections",',
    "list owner operation label",
  ],
  ['"list_order_returns",', "list consumer operation label"],
  ["PaginationMeta::new(pagination.page, pagination.limit(), page.total)", "list pagination meta"],
]) requireText(listReturnsRoute, value, `list order returns route: ${label}`);

for (const [value, label] of [
  ["pub async fn show_order_return(", "show return handler"],
  ['"invalid_order_return_id"', "show return id guard"],
  ["ReadOrderReturnProjectionRequest { return_id: id }", "typed show-return request"],
  [".read_order_return_projection(", "show owner read"],
  ['"read_order_return_projection",', "show owner operation label"],
  ['"show_order_return",', "show consumer operation label"],
  ["Ok(Json(item))", "show return response"],
]) requireText(showReturnRoute, value, `show order return route: ${label}`);

for (const [value, label] of [
  ["pub(crate) fn order_error_policy(", "shared order policy"],
  ["pub(crate) fn payment_error_policy(", "shared payment policy"],
  ["pub(crate) fn reserved_refund_error_policy(", "shared reserved refund policy"],
  ["pub(crate) type AdminErrorPolicy", "shared admin policy type"],
]) requireText(adminMod, value, label);

// --- Retired conversions ------------------------------------------------------------------------

for (const [content, label] of [
  [commands, "post_order_commands.rs"],
  [reads, "post_order_reads.rs"],
]) {
  for (const forbidden of [
    ".map_err(|_| HttpError::internal(",
    "err.to_string()",
    "error.to_string()",
    "error.message",
  ]) {
    forbidText(content, forbidden, `${label}: owner text leak`);
  }
}

if (failures.length > 0) {
  console.error("Commerce admin order-return owner error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order-return routes keep the host-selected Order owner ports, static envelopes, and bounded diagnostics",
);
