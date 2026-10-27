#!/usr/bin/env node

// Admin order-detail payment error context.
//
// `GET /admin/orders/{id}` composes three owner reads. The payment half reads the latest payment
// collection for the order through the host-selected Payment order-read port, and a failure there
// must not be reported as an order failure: it has its own owner label, its own boundary, and its
// own envelope set, and an invariant violation means the payment state needs reconciliation before it
// can be read — not that storage is down.
//
// The verifier used to describe the retired `admin/orders.rs` implementation, where the detail route
// called `PaymentService::find_latest_collection_by_order(tenant.id, id)` in-process, mapped a typed
// `PaymentError` with `map_order_detail_payment_error(tenant.id, id, error)`, logged `error = ?error`
// with `tenant_id = %tenant_id` / `order_id = %order_id`, and kept eleven `PaymentError` arms. The
// live module reads through `payment_order_read_port()` and maps a `PortError`, so the guard is
// re-anchored on that contract: one owner label, one boundary, seven port kinds, static envelopes,
// and bounded facts.

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
  "const ADMIN_ORDER_DETAIL_PAYMENT_OWNER:",
  "const ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER:",
  "order detail payment constants",
);
const mapper = between(
  controller,
  "fn map_payment_detail_port_error(",
  "fn map_fulfillment_detail_port_error(",
  "order detail payment mapper",
);
const showRoute = between(
  controller,
  "pub async fn show_order(",
  "pub async fn mark_order_paid(",
  "show order route",
);
const paymentRead = between(
  showRoute,
  "let payment_context =",
  "let fulfillment_context =",
  "order detail payment read",
);

requireText(
  constants,
  'const ADMIN_ORDER_DETAIL_PAYMENT_OWNER: &str = "rustok_payment.admin_order_detail";',
  "payment detail owner label",
);
requireText(
  controller,
  'const ADMIN_ORDER_DETAIL_BOUNDARY: &str = "commerce_admin_order_detail_http";',
  "order detail boundary",
);

// --- Payment owner mapper ----------------------------------------------------------------------

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const variant of portErrorKinds) {
  requirePattern(
    mapper,
    new RegExp(`PortErrorKind::${variant}\\b`),
    `payment detail mapper handles PortErrorKind::${variant}`,
  );
}
forbidText(mapper, "        _ =>", "wildcard in the payment detail mapper");

for (const [value, label] of [
  ["StatusCode::BAD_REQUEST", "validation status"],
  ['"commerce_admin_payment_invalid"', "validation code"],
  ['"Payment request is invalid"', "validation message"],
  ["StatusCode::NOT_FOUND", "not-found status"],
  ['"commerce_admin_not_found"', "not-found code"],
  ['"Commerce resource not found"', "not-found message"],
  ["StatusCode::CONFLICT", "conflict status"],
  ['"commerce_admin_payment_state_conflict"', "conflict code"],
  ['"Payment operation conflicts with the current state"', "conflict message"],
  ["StatusCode::FORBIDDEN", "forbidden status"],
  ['"commerce_permission_denied"', "forbidden code"],
  ['"Permission denied"', "forbidden message"],
  ["StatusCode::SERVICE_UNAVAILABLE", "storage status"],
  ['"commerce_admin_payment_storage_unavailable"', "storage code"],
  ['"Payment storage is temporarily unavailable"', "storage message"],
  [
    '"commerce_admin_payment_reconciliation_required"',
    "invariant reconciliation code",
  ],
  [
    '"Payment state requires reconciliation before it can be read safely"',
    "invariant reconciliation message",
  ],
  ["HttpError::new(status, code, message)", "single static envelope constructor"],
]) requireText(mapper, value, label);

// An invariant violation on the payment read is a reconciliation fact, not a storage outage.
requireTextNormalized(
  mapper,
  'PortErrorKind::InvariantViolation => ( StatusCode::CONFLICT, "commerce_admin_payment_reconciliation_required", "Payment state requires reconciliation before it can be read safely", )',
  "invariant violation maps to a reconciliation conflict",
);

for (const [value, label] of [
  ["owner = ADMIN_ORDER_DETAIL_PAYMENT_OWNER,", "payment owner field"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
  ["order_id_non_nil = !order_id.is_nil(),", "order shape"],
  ['operation = "find_latest_payment_collection_by_order",', "owner operation label"],
  ["correlation_id = %context.correlation_id,", "correlation join key"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
  ["retryable = error.retryable,", "retryability"],
  ["public_code = code,", "public code"],
  ["status = %status,", "status"],
  ["boundary = ADMIN_ORDER_DETAIL_BOUNDARY,", "detail boundary"],
  ['"commerce admin order detail payment owner read failed"', "mapper message"],
  ["let error_kind = port_error_kind(&error);", "typed error kind projection"],
]) requireText(mapper, value, label);

for (const forbidden of ["error = ?error", "%error", "error.message", ".to_string()", "format!("]) {
  forbidText(mapper, forbidden, "raw payment detail payload");
}
forbidRawContextLogging(failures, controller, "admin order controller");

// --- Detail route payment read -----------------------------------------------------------------

for (const [value, label] of [
  ["let payment_context =", "payment port context"],
  [
    'admin_order_port_context(&tenant, &auth, &request_context, Some(id), "payment_detail")',
    "payment detail operation label",
  ],
  ["runtime", "host runtime"],
  [".payment_order_read_port()", "host-selected Payment order-read port"],
  [".find_latest_collection_by_order(", "payment collection owner read"],
  ["payment_context.clone(),", "payment context handoff"],
  ["LatestPaymentCollectionByOrderRequest { order_id: id }", "typed payment read request"],
  [".await", "payment read await"],
  [".map_err(|error| map_payment_detail_port_error(tenant.id, id, &payment_context, error))?;",
   "payment detail mapper handoff"],
]) requireTextNormalized(paymentRead, value, `payment read: ${label}`);

requireOrdered(
  showRoute,
  [
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_entry(",
    ".read_order_projection(",
    "let payment_context =",
    ".find_latest_collection_by_order(",
    "let fulfillment_context =",
    "Ok(Json(AdminOrderDetailResponse {",
  ],
  "detail route order",
);

for (const [value, label] of [
  ["payment_collection", "payment collection in the response"],
  ["fulfillment", "fulfillment in the response"],
]) requireText(showRoute, value, `detail response: ${label}`);

requireCount(
  controller,
  /map_payment_detail_port_error\(/g,
  2,
  "one payment detail mapper and one callsite",
);

// --- Retired in-process payment path -----------------------------------------------------------

for (const forbidden of [
  "PaymentService::new(",
  "map_order_detail_payment_error(",
  "PaymentError::ProviderOutcomeUnknown",
  "find_latest_collection_by_order(tenant.id, id)",
]) forbidText(controller, forbidden, "retired in-process payment detail path");

if (failures.length > 0) {
  console.error("Commerce admin order-detail payment error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order detail reads the payment collection through the host-selected owner port with its own envelope set and bounded diagnostics",
);
