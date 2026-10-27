#!/usr/bin/env node

// Admin order route error context.
//
// The admin order surface (`orders_owner_ports.rs`, mounted as `admin::orders`) reads and mutates
// orders through host-selected owner ports. `/admin/orders` lists via the Order read port and
// `/admin/orders/{id}` composes three owner reads — Order, Payment, and Fulfillment — into one
// detail response. Every one of those reads can fail with a `PortError`, so each has its own mapper
// that selects a static Commerce envelope and records only bounded facts.
//
// The verifier used to describe the retired direct-service implementation in `admin/orders.rs`: an
// `AdminOrderErrorContext` built per route, `admin_order_read_port_context`, `map_admin_order_port_error`,
// `map_order_detail_payment_error` and `map_order_detail_fulfillment_error`, all with
// `error = ?error` and raw identity fields. That file is no longer part of the module tree, and the
// live module splits the detail read into three owner-specific mappers. The guard is re-anchored on
// the live contract.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const controller = read(
  "crates/modules/rustok-commerce/src/controllers/admin/orders_owner_ports.rs",
);
const moduleTree = read("crates/modules/rustok-commerce/src/controllers/admin/mod.rs");
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

const constants = between(
  controller,
  "const ADMIN_ORDER_OWNER:",
  "fn require_idempotency_key(",
  "admin order constants",
);
const entryLogger = between(
  controller,
  "fn log_admin_order_entry(",
  "fn require_idempotency_key(",
  "admin order entry logger",
);
const portContext = between(
  controller,
  "fn admin_order_port_context(",
  "fn admin_order_command_port_context(",
  "admin order port context",
);
const commandPortContext = between(
  controller,
  "fn admin_order_command_port_context(",
  "fn port_error_kind(",
  "admin order command port context",
);
const kindProjection = between(
  controller,
  "fn port_error_kind(",
  "fn map_order_port_error(",
  "port error kind projection",
);
const orderMapper = between(
  controller,
  "fn map_order_port_error(",
  "fn map_payment_detail_port_error(",
  "admin order port mapper",
);
const listRoute = between(
  controller,
  "pub async fn list_orders(",
  "pub async fn show_order(",
  "list orders route",
);
const showRoute = between(
  controller,
  "pub async fn show_order(",
  "pub async fn mark_order_paid(",
  "show order route",
);

// --- Module wiring -----------------------------------------------------------------------------

requireText(
  moduleTree,
  '#[path = "orders_owner_ports.rs"]',
  "admin::orders is mounted from orders_owner_ports.rs",
);

for (const [value, label] of [
  ['const ADMIN_ORDER_OWNER: &str = "rustok_order.admin_orders";', "order owner"],
  ['const ADMIN_ORDER_BOUNDARY: &str = "commerce_admin_order_http";', "order boundary"],
  [
    'const ADMIN_ORDER_DETAIL_PAYMENT_OWNER: &str = "rustok_payment.admin_order_detail";',
    "payment detail owner",
  ],
  [
    'const ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_order_detail";',
    "fulfillment detail owner",
  ],
  [
    'const ADMIN_ORDER_DETAIL_BOUNDARY: &str = "commerce_admin_order_detail_http";',
    "detail boundary",
  ],
]) requireText(constants, value, label);

// --- Bounded entry line ------------------------------------------------------------------------

for (const [value, label] of [
  ["operation: &'static str,", "entry operation"],
  ["order_id: Option<Uuid>,", "entry order identity"],
  ["status_filter_present: bool,", "entry status filter"],
  ["customer_filter_present: bool,", "entry customer filter"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "entry actor shape"],
  ["order_id_present = order_id.is_some(),", "entry order presence"],
  ["order_id_non_nil = order_id.is_some_and(|value| !value.is_nil()),", "entry order shape"],
  ["status_filter_present,", "entry status filter field"],
  ["customer_filter_present,", "entry customer filter field"],
  ["boundary = ADMIN_ORDER_BOUNDARY,", "entry boundary"],
  ['"commerce admin order route entered with bounded diagnostics"', "entry message"],
]) requireText(entryLogger, value, label);

requireCount(controller, /log_admin_order_entry\(\n        "/g, 6, "six bounded entry lines");
requireCount(controller, /tracing::debug!\(/g, 1, "one debug line");
requireCount(controller, /tracing::error!\(/g, 3, "three owner mapper error lines");
requireCount(controller, /tracing::warn!\(/g, 0, "no warn line");

forbidRawContextLogging(failures, controller, "admin order controller");
for (const forbidden of [
  "AdminOrderErrorContext::new(",
  "admin_order_read_port_context(",
  "map_admin_order_port_error(",
  "fn map_order_detail_payment_error(",
  "fn map_order_detail_fulfillment_error(",
  "error = ?error",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
]) forbidText(controller, forbidden, "retired admin order route machinery");

// --- Port contexts -----------------------------------------------------------------------------

for (const [value, label] of [
  ["let resource_id = order_id.unwrap_or(tenant.id);", "bounded resource identity"],
  ["PortContext::new(", "port context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "actor projection"],
  ["request_context.locale.as_str(),", "locale projection"],
  ['format!("commerce-admin-order:{operation}:{resource_id}")', "correlation key"],
]) requireText(portContext, value, label);

for (const [value, label] of [
  [
    "admin_order_port_context(tenant, auth, request_context, Some(order_id), operation)",
    "command context delegates to the shared port context",
  ],
  [".with_idempotency_key(idempotency_key)", "command idempotency key"],
  ["idempotency_key: String,", "command idempotency input"],
]) requireTextNormalized(commandPortContext, value, label);

// --- Order port policy -------------------------------------------------------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const variant of portErrorKinds) {
  requirePattern(
    kindProjection,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `port error kind projection handles ${variant}`,
  );
  requirePattern(
    orderMapper,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `admin order mapper handles ${variant}`,
  );
}

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
  ['"Permission denied"', "forbidden message"],
  ["StatusCode::SERVICE_UNAVAILABLE", "storage status"],
  ['"commerce_admin_order_storage_unavailable"', "storage code"],
  ['"Order storage is temporarily unavailable"', "storage message"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "fail-closed status"],
  ['"commerce_admin_order_failed"', "fail-closed code"],
  ['"Order operation could not be completed safely"', "fail-closed message"],
  ["HttpError::new(status, code, message)", "single static envelope constructor"],
]) requireText(orderMapper, value, label);

for (const [value, label] of [
  ["owner = ADMIN_ORDER_OWNER,", "owner field"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape"],
  ["order_id_present = order_id.is_some(),", "order presence"],
  ["operation,", "route operation"],
  ["correlation_id = %context.correlation_id,", "correlation join key"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
  ["retryable = error.retryable,", "retryability"],
  ["public_code = code,", "public code"],
  ["status = %status,", "status"],
  ["boundary = ADMIN_ORDER_BOUNDARY,", "boundary"],
  ['"commerce admin order owner port failed"', "mapper message"],
]) requireText(orderMapper, value, label);

// --- Read routes -------------------------------------------------------------------------------

for (const [value, label] of [
  ["ensure_permissions(", "permission gate"],
  ["[Permission::ORDERS_LIST]", "list permission"],
  ['"Permission denied: orders:list required"', "list permission message"],
  ['"commerce_invalid_tenant"', "tenant guard"],
  ["log_admin_order_entry(", "bounded entry line"],
  ['"list_orders",', "list operation label"],
  ["admin_order_port_context(&tenant, &auth, &request_context, None, \"list_orders\")",
   "list port context"],
  [".order_read_port()", "host-selected Order read port"],
  [".list_order_projections(", "list owner read"],
  ["ListOrderProjectionsRequest {", "typed list request"],
  ["page: pagination.page,", "list page forwarding"],
  ["per_page: pagination.limit(),", "list page-size forwarding"],
  ["status: params.status,", "list status filter"],
  ["customer_id,", "list customer filter"],
  ["tenant_default_locale: Some(tenant.default_locale.clone()),", "locale fallback"],
  [".map_err(|error| {", "typed mapping closure"],
  ["map_order_port_error(", "order port mapper handoff"],
  [
    "super::super::common::PaginationMeta::new( pagination.page, pagination.limit(), page.total, )",
    "list pagination meta",
  ],
]) requireTextNormalized(listRoute, value, `list orders route: ${label}`);

for (const [value, label] of [
  ["ensure_permissions(", "permission gate"],
  ["[Permission::ORDERS_READ]", "read permission"],
  ['"Permission denied: orders:read required"', "read permission message"],
  ['"commerce_invalid_tenant"', "tenant guard"],
  ['"commerce_invalid_id"', "id guard"],
  ["log_admin_order_entry(", "bounded entry line"],
  ['"show_order",', "show operation label"],
  ['admin_order_port_context(&tenant, &auth, &request_context, Some(id), "get_order")',
   "order read port context"],
  [".order_read_port()", "host-selected Order read port"],
  [".read_order_projection(", "detail owner read"],
  ["ReadOrderProjectionRequest {", "typed detail request"],
  ["order_id: id,", "detail order identity"],
  ["tenant_default_locale: Some(tenant.default_locale.clone()),", "detail locale fallback"],
  ['"get_order",', "order read operation label"],
  ["Ok(Json(AdminOrderDetailResponse {", "detail response"],
  ["payment_collection,", "payment collection in the response"],
  ["fulfillment,", "fulfillment in the response"],
]) requireTextNormalized(showRoute, value, `show order route: ${label}`);

requireOrdered(
  showRoute,
  [
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_entry(",
    ".read_order_projection(",
    ".map_err(|error| {",
  ],
  "show order route order",
);

if (failures.length > 0) {
  console.error("Commerce admin order route error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order routes read every owner through a host-selected port, map PortError onto static envelopes, and log shape only",
);
