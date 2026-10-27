#!/usr/bin/env node

// Active admin order/fulfillment HTTP boundary.
//
// Every admin order and fulfillment route speaks to another module through a host-selected port:
// Order (read, post-order command, admin command), Payment (order read), Fulfillment (read, admin
// command, admin create command). Two invariants hold across that whole surface, and this guard
// exists to keep them:
//
//   1. every write handler enforces a caller-owned `Idempotency-Key` and forwards it into the port
//      context, so a retried admin request cannot execute twice;
//   2. every entry and failure line records shape, not caller identity or owner text.
//
// The verifier used to pin a fixed number of handlers per file (`4` order writes, `6` fulfillment
// writes) and to look for the order-change and order-return handlers inside `changes.rs` and
// `returns.rs`. Those handlers moved to `post_order_commands.rs` (create/cancel) and
// `post_order_reads.rs` (list/show), and the fulfillment `.with_idempotency_key` check used an
// unescaped regular expression that could never match. The guard now derives the handler map from
// the files that own them and checks each file's header/idempotency counts against each other, so a
// dropped enforcement fails here with the file name.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const base = "crates/modules/rustok-commerce/src/controllers/admin";
const admin = read(`${base}/mod.rs`);
const orders = read(`${base}/orders_owner_ports.rs`);
const changes = read(`${base}/changes.rs`);
const returns = read(`${base}/returns.rs`);
const orderCommands = read(`${base}/post_order_commands.rs`);
const orderReads = read(`${base}/post_order_reads.rs`);
const fulfillments = read(`${base}/fulfillments_owner_commands.rs`);
const orderAdminCommand = read("crates/modules/rustok-order/src/admin_command.rs");
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, pattern, expected, label) => {
  const actual = content.match(pattern)?.length ?? 0;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};
const countOf = (content, literal) => content.split(literal).length - 1;
const normalize = (content) => content.replace(/\s+/g, " ");
const requireTextNormalized = (content, value, label) => {
  if (!normalize(content).includes(normalize(value))) failures.push(`${label}: missing ${value}`);
};

// --- Module wiring ------------------------------------------------------------------------------

for (const [value, label] of [
  ['#[path = "orders_owner_ports.rs"]', "admin order module is served by the owner-port file"],
  ["pub mod fulfillments;", "fulfillment command module"],
  ["pub mod post_order_commands;", "post-order command module"],
  ["pub mod post_order_reads;", "post-order read module"],
  ["pub mod changes;", "order-change module"],
  ["pub mod returns;", "order-return module"],
]) requireText(admin, value, `admin module wiring: ${label}`);

// --- Handler map, derived from the files that actually own the handlers -------------------------

const handlers = [
  [
    orders,
    "admin order controller",
    [
      ["pub async fn list_orders(", "list orders"],
      ["pub async fn show_order(", "show order"],
      ["pub async fn mark_order_paid(", "mark order paid"],
      ["pub async fn ship_order(", "ship order"],
      ["pub async fn deliver_order(", "deliver order"],
      ["pub async fn cancel_order(", "cancel order"],
    ],
  ],
  [
    orderCommands,
    "post-order command controller",
    [
      ["pub async fn create_order_change(", "create order change"],
      ["pub async fn cancel_order_change(", "cancel order change"],
      ["pub async fn create_order_return(", "create order return"],
      ["pub async fn cancel_order_return(", "cancel order return"],
    ],
  ],
  [
    orderReads,
    "post-order read controller",
    [
      ["pub async fn list_order_returns(", "list order returns"],
      ["pub async fn show_order_return(", "show order return"],
      ["pub async fn list_order_changes(", "list order changes"],
      ["pub async fn show_order_change(", "show order change"],
    ],
  ],
  [
    changes,
    "order-change controller",
    [["pub async fn apply_order_change(", "apply order change"]],
  ],
  [
    returns,
    "order-return controller",
    [
      ["pub async fn create_order_return_decision(", "create return decision"],
      ["pub async fn complete_order_return(", "complete order return"],
    ],
  ],
  [
    fulfillments,
    "fulfillment controller",
    [
      ["pub async fn list_fulfillments(", "list fulfillments"],
      ["pub async fn show_fulfillment(", "show fulfillment"],
      ["pub async fn create_fulfillment(", "create fulfillment"],
      ["pub async fn ship_fulfillment(", "ship fulfillment"],
      ["pub async fn deliver_fulfillment(", "deliver fulfillment"],
      ["pub async fn reopen_fulfillment(", "reopen fulfillment"],
      ["pub async fn reship_fulfillment(", "reship fulfillment"],
      ["pub async fn cancel_fulfillment(", "cancel fulfillment"],
    ],
  ],
];

for (const [content, label, entries] of handlers) {
  for (const [value, name] of entries) requireText(content, value, `${label}: ${name}`);
}

// --- Caller-owned idempotency -------------------------------------------------------------------
//
// The invariant is per file: every handler that extracts `headers: HeaderMap` must also require the
// key, and the expected number is pinned so a handler cannot quietly stop extracting headers.

const idempotencyExpectations = [
  [orders, "admin order controller", 4],
  [orderCommands, "post-order command controller", 4],
  [changes, "order-change controller", 1],
  [returns, "order-return controller", 1],
  [fulfillments, "fulfillment controller", 6],
];

for (const [content, label, expected] of idempotencyExpectations) {
  const headerCount = countOf(content, "headers: HeaderMap,");
  const requireCountValue = countOf(content, "let idempotency_key = require_idempotency_key(&headers)?;");
  if (headerCount !== expected) {
    failures.push(`${label}: expected ${expected} idempotent writes, found ${headerCount} header extractions`);
  }
  if (requireCountValue !== headerCount) {
    failures.push(
      `${label}: ${headerCount} write handlers extract Idempotency-Key but only ${requireCountValue} require it`,
    );
  }
  requireText(content, "fn require_idempotency_key(headers: &HeaderMap)", `${label}: idempotency parser`);
}

for (const [value, label] of [
  [".with_idempotency_key(idempotency_key)", "command context propagation"],
  ["idempotency_key: String,", "command context input"],
]) requireText(orders, value, `admin order port context: ${label}`);

requireCount(
  fulfillments,
  /\.with_idempotency_key\(idempotency_key\)/g,
  2,
  "fulfillment create and command contexts propagate the caller-owned key",
);
for (const [value, label] of [
  ["fn admin_fulfillment_command_context(", "fulfillment command context"],
  ["fn admin_fulfillment_create_read_context(", "fulfillment create read context"],
  ["fn admin_fulfillment_create_command_context(", "fulfillment create command context"],
]) requireText(fulfillments, value, `fulfillment contexts: ${label}`);

// Each of these controllers owns one context builder; the per-route guarantee is that every write
// handler hands its parsed key to that builder.
for (const [content, label] of [
  [changes, "order-change controller"],
  [returns, "order-return controller"],
  [orderCommands, "post-order command controller"],
]) {
  requireCount(
    content,
    /\.with_idempotency_key\(idempotency_key\)/g,
    1,
    `${label}: the port context carries the caller-owned key`,
  );
  requireText(content, "idempotency_key: String,", `${label}: builder accepts the caller-owned key`);
}

requireCount(
  orderCommands,
  /^\s+idempotency_key,$/gm,
  4,
  "post-order command controller: every write forwards its key to the builder",
);
requireTextNormalized(
  changes,
  "admin_order_change_apply_context(&tenant, &auth, &request_context, id, idempotency_key)",
  "order-change controller: apply route forwards its key to the builder",
);
requireTextNormalized(
  returns,
  "admin_return_decision_order_context(&tenant, &auth, &request_context, id, idempotency_key)",
  "order-return controller: decision route forwards its key to the builder",
);

// --- Owner ports, not in-process services -------------------------------------------------------

for (const [value, label] of [
  ["fn admin_order_port_context(", "read port context"],
  ["fn admin_order_command_port_context(", "command port context"],
  [".order_read_port()", "order read owner port"],
  [".payment_order_read_port()", "payment detail owner port"],
  [".fulfillment_read_port()", "fulfillment detail owner port"],
  [".order_admin_command_port()", "order command owner port"],
  ["OwnerMarkOrderPaidRequest {", "typed mark-paid request"],
  ["OwnerShipOrderRequest {", "typed ship request"],
  ["OwnerDeliverOrderRequest {", "typed deliver request"],
  ["OwnerCancelOrderRequest {", "typed cancel request"],
]) requireText(orders, value, label);

for (const [value, label] of [
  [".fulfillment_read_port()", "fulfillment read owner port"],
  [".fulfillment_admin_command_port()", "fulfillment command owner port"],
  [".fulfillment_admin_create_command_port()", "fulfillment create owner port"],
  [".shipping_option_read_port()", "shipping option owner port"],
  ["ShipAdminFulfillmentRequest {", "typed ship request"],
]) requireText(fulfillments, value, `fulfillment owner ports: ${label}`);

for (const [content, label] of [
  [orders, "admin order controller"],
  [fulfillments, "fulfillment controller"],
  [orderCommands, "post-order command controller"],
  [orderReads, "post-order read controller"],
  [changes, "order-change controller"],
  [returns, "order-return controller"],
]) {
  for (const value of [
    "OrderService::new(",
    "PaymentService::new(",
    "FulfillmentService::new(",
    "Uuid::new_v4().to_string()",
    "admin_public_error(",
  ]) forbidText(content, value, `${label}: in-process owner service or generated identity`);
}

// --- Bounded entry lines ------------------------------------------------------------------------

for (const [value, label] of [
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape fact"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape fact"],
  ["order_id_present = order_id.is_some(),", "order presence fact"],
  ["order_id_non_nil = order_id.is_some_and(|value| !value.is_nil()),", "order shape fact"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code fact"],
  ["retryable = error.retryable,", "retryability fact"],
  ["public_code = code,", "stable public code fact"],
  ["boundary = ADMIN_ORDER_BOUNDARY,", "order boundary"],
]) requireText(orders, value, `admin order diagnostics: ${label}`);

for (const [value, label] of [
  ["fn log_admin_fulfillment_route_entry(", "bounded entry helper"],
  ["struct AdminFulfillmentRouteFacts {", "bounded entry facts"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape fact"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape fact"],
  ["fulfillment_id_present = facts.fulfillment_id.is_some(),", "fulfillment presence fact"],
  [
    "fulfillment_id_non_nil = facts.fulfillment_id.is_some_and(|value| !value.is_nil()),",
    "fulfillment shape fact",
  ],
  ["order_id_non_nil = facts.order_id.is_some_and(|value| !value.is_nil()),", "order shape fact"],
  ["status_filter_present = facts.status_filter_present,", "status filter fact"],
  ["customer_filter_present = facts.customer_filter_present,", "customer filter fact"],
  ["carrier_length = facts.carrier_length.unwrap_or_default(),", "bounded carrier length"],
  ["boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,", "fulfillment boundary"],
  ["owner = ADMIN_FULFILLMENT_COMMAND_OWNER,", "fulfillment command owner"],
  ["owner = ADMIN_FULFILLMENT_CREATE_OWNER,", "fulfillment create owner"],
  ["owner_code_length = error.code.chars().count(),", "bounded owner code fact"],
  ["retryable = error.retryable,", "retryability fact"],
]) requireText(fulfillments, value, `fulfillment diagnostics: ${label}`);

requireCount(
  fulfillments,
  /log_admin_fulfillment_route_entry\(\n        "/g,
  8,
  "fulfillment controller: eight bounded entry lines",
);
requireCount(fulfillments, /tracing::debug!\(/g, 1, "fulfillment controller: one debug line");
requireCount(fulfillments, /tracing::error!\(/g, 3, "fulfillment controller: three error lines");

for (const [content, label] of [
  [orders, "admin order controller"],
  [fulfillments, "fulfillment controller"],
  [orderCommands, "post-order command controller"],
  [orderReads, "post-order read controller"],
  [changes, "order-change controller"],
  [returns, "order-return controller"],
]) {
  forbidRawContextLogging(failures, content, label);
  for (const value of [
    "error = ?error",
    "internal_message = %error.message",
    "internal_code = %error.code",
    "E: std::fmt::Debug",
    "err.to_string()",
    "error.to_string()",
    "tenant_id = %context.tenant_id",
    "tenant_id = %tenant.id",
  ]) forbidText(content, value, `${label}: raw diagnostic directive`);
}

// --- Owner command port -------------------------------------------------------------------------

for (const [value, label] of [
  ["pub trait OrderAdminCommandPort", "owner command port trait"],
  ["fn require_admin_command_context(", "owner command context validation"],
]) requireText(orderAdminCommand, value, `rustok-order admin command: ${label}`);

if (failures.length > 0) {
  console.error("Commerce admin order/fulfillment HTTP error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order and fulfillment writes require caller-owned idempotency, use host-selected owner ports, and log shape only",
);
