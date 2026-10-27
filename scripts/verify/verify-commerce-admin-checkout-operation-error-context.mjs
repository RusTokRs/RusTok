#!/usr/bin/env node

// Admin checkout-operation error context.
//
// Every admin checkout-operation route — show, compensate, list, reconciliation execute, action
// list, compensation sweep — runs an owner service that can fail in a payment, order, inventory,
// cart, journal, or storage layer. The route must never let one of those failures reach the operator
// as owner text or as a raw identity value: each owner failure is projected onto a static envelope
// by a policy function, and the diagnostic line records the *shape* of the identity it was handed.
//
// The verifier used to describe the previous implementation: a per-owner policy set declared inside
// the controller (`payment_error_policy`, `reserved_refund_error_policy`, `order_error_policy`), a
// generic `admin_checkout_operation_http_error<E>` logger, `error = ?error`, and raw
// `tenant_id = %context.tenant_id` / `actor_id = %context.actor_id` /
// `checkout_operation_id = ?context.checkout_operation_id` log fields. Those policies now live in
// `controllers/admin/mod.rs` and are shared with the other admin mappers, the logger is
// non-generic and emits `*_state` shapes, and the route-entry lines are bounded helpers. The guard
// is written against that contract.
//
// Two checks are deliberately derived from the owner enums rather than hand-listed, because the
// regression this guard missed was a mapper that silently lost three variants when the policies
// moved: `CheckoutCompensationError` grew `Payment`, `PaymentOrchestration`, and `Order` arms that
// `map_compensation_error` never regained, leaving the match non-exhaustive and the operator
// without an envelope for a payment or order compensation failure. Every owner variant must appear
// in its policy or mapper, and no policy may paper over a new variant with a wildcard arm.

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
  "crates/modules/rustok-commerce/src/controllers/admin/checkout_operations.rs",
);
const adminMod = read("crates/modules/rustok-commerce/src/controllers/admin/mod.rs");
const sweep = read(
  "crates/modules/rustok-commerce/src/services/checkout_compensation_sweep.rs",
);
const operationErrors = read(
  "crates/modules/rustok-commerce/src/services/checkout_operation.rs",
);
const compensationErrors = read(
  "crates/modules/rustok-commerce/src/services/checkout_compensation.rs",
);
const reservationErrors = read(
  "crates/modules/rustok-commerce/src/services/checkout_inventory_reservation_journal.rs",
);
const paymentErrors = read("crates/modules/rustok-payment/src/error.rs");
const orderErrors = read("crates/modules/rustok-order/src/error.rs");
const paymentOrchestration = read(
  "crates/modules/rustok-commerce/src/services/payment_orchestration.rs",
);
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

// Owner enums are the source of truth for "did the mapper actually handle this failure".
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
const requireEveryVariant = (variants, content, enumName, target, label) => {
  for (const variant of variants) {
    requirePattern(
      content,
      new RegExp(`${enumName}::${variant}\\b`),
      `${label} handles ${enumName}::${variant}`,
    );
  }
};

const controllerImports = between(controller, "use super::{", "use crate::{", "controller imports");
const boundaryConstants = between(
  controller,
  "const ADMIN_CHECKOUT_OPERATION_OWNER:",
  "type AdminCheckoutOperationHttpPolicy",
  "checkout operation boundary constants",
);
const policyAlias = between(
  controller,
  "type AdminCheckoutOperationHttpPolicy",
  "struct AdminCheckoutOperationErrorContext",
  "checkout operation policy alias",
);
const errorContext = between(
  controller,
  "struct AdminCheckoutOperationErrorContext {",
  "impl AdminCheckoutOperationErrorContext {",
  "checkout operation error context",
);
const diagnosticContextType = between(
  controller,
  "struct AdminCheckoutOperationDiagnosticContext {",
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "checkout operation diagnostic context type",
);
const diagnosticContext = between(
  controller,
  "impl From<&AdminCheckoutOperationErrorContext> for AdminCheckoutOperationDiagnosticContext {",
  "fn uuid_shape(",
  "checkout operation diagnostic context",
);
const shapeHelpers = between(
  controller,
  "fn uuid_shape(",
  "struct AdminCheckoutOperationRouteFacts",
  "checkout operation shape helpers",
);
const routeEntryLogger = between(
  controller,
  "fn log_admin_checkout_operation_route_entry(",
  "fn log_admin_checkout_reconciliation_recorded(",
  "checkout operation route entry logger",
);
const recordedLogger = between(
  controller,
  "fn log_admin_checkout_reconciliation_recorded(",
  "pub struct AdminCheckoutOperationResponse {",
  "checkout reconciliation recorded logger",
);
const showRoute = between(
  controller,
  "pub async fn show_checkout_operation(",
  "pub async fn compensate_checkout_operation(",
  "show checkout operation route",
);
const compensateRoute = between(
  controller,
  "pub async fn compensate_checkout_operation(",
  "pub async fn list_checkout_operations(",
  "compensate checkout operation route",
);
const listRoute = between(
  controller,
  "pub async fn list_checkout_operations(",
  "pub async fn execute_checkout_reconciliation_action(",
  "list checkout operations route",
);
const reconciliationActionRoute = between(
  controller,
  "pub async fn execute_checkout_reconciliation_action(",
  "pub async fn list_checkout_reconciliation_actions(",
  "checkout reconciliation action route",
);
const listActionsRoute = between(
  controller,
  "pub async fn list_checkout_reconciliation_actions(",
  "pub async fn sweep_checkout_compensations(",
  "checkout reconciliation action list route",
);
const sweepRoute = between(
  controller,
  "pub async fn sweep_checkout_compensations(",
  "fn map_operation(",
  "checkout compensation sweep route",
);
const reconciliationErrorMapper = between(
  controller,
  "fn map_reconciliation_error(",
  "fn reconciliation_error_policy(",
  "checkout reconciliation error mapper",
);
const reconciliationPolicy = between(
  controller,
  "fn reconciliation_error_policy(",
  "fn checkout_operation_error_policy(",
  "checkout reconciliation error policy",
);
const operationPolicy = between(
  controller,
  "fn checkout_operation_error_policy(",
  "fn adopt_operation_error_identity(",
  "checkout operation error policy",
);
const identityAdoption = between(
  controller,
  "fn adopt_operation_error_identity(",
  "fn admin_checkout_operation_http_error(",
  "checkout operation identity adoption",
);
const httpErrorLogger = between(
  controller,
  "fn admin_checkout_operation_http_error(",
  "fn map_operation_error(",
  "checkout operation http error logger",
);
const operationMapper = between(
  controller,
  "fn map_operation_error(",
  "fn map_compensation_error(",
  "checkout operation mapper",
);
const compensationMapper = between(
  controller,
  "fn map_compensation_error(",
  "fn compensation_boundary_owner(",
  "checkout compensation mapper",
);
const boundaryOwner = between(
  controller,
  "fn compensation_boundary_owner(",
  "fn map_sweep_error(",
  "checkout compensation boundary owner",
);
const sweepMapper = from(controller, "fn map_sweep_error(", "checkout sweep mapper");

const paymentPolicy = between(
  adminMod,
  "pub(crate) fn payment_error_policy(",
  "pub(crate) fn reserved_refund_error_policy(",
  "shared admin payment policy",
);
const reservedRefundPolicy = between(
  adminMod,
  "pub(crate) fn reserved_refund_error_policy(",
  "pub(crate) fn payment_orchestration_error_policy(",
  "shared admin reserved refund policy",
);
const orchestrationPolicy = between(
  adminMod,
  "pub(crate) fn payment_orchestration_error_policy(",
  "pub(crate) fn map_payment_orchestration_error(",
  "shared admin payment orchestration policy",
);
const orderPolicy = between(
  adminMod,
  "pub(crate) fn order_error_policy(",
  "pub(crate) fn map_order_error(",
  "shared admin order policy",
);
const orderMapper = between(
  adminMod,
  "pub(crate) fn map_order_error(",
  "pub(crate) fn map_post_order_orchestration_error(",
  "shared admin order mapper",
);

// --- Boundary constants, context, and shared policy wiring --------------------------------------

for (const [value, label] of [
  [
    'const ADMIN_CHECKOUT_OPERATION_OWNER: &str = "rustok_commerce.admin_checkout_operation";',
    "owner constant",
  ],
  [
    'const ADMIN_CHECKOUT_OPERATION_BOUNDARY: &str = "commerce_admin_checkout_operation_http";',
    "HTTP boundary constant",
  ],
]) requireText(boundaryConstants, value, label);

requireText(
  policyAlias,
  "type AdminCheckoutOperationHttpPolicy = AdminErrorPolicy;",
  "checkout policy alias reuses the shared admin policy shape",
);

for (const [value, label] of [
  ["tenant_id: Uuid,", "tenant field"],
  ["actor_id: Uuid,", "actor field"],
  ["checkout_operation_id: Option<Uuid>,", "checkout operation identity field"],
  ["reservation_id: Option<Uuid>,", "reservation identity field"],
  ["payment_collection_id: Option<Uuid>,", "payment collection identity field"],
  ["payment_id: Option<Uuid>,", "payment identity field"],
  ["refund_id: Option<Uuid>,", "refund identity field"],
  ["order_id: Option<Uuid>,", "order identity field"],
  ["order_return_id: Option<Uuid>,", "order return identity field"],
  ["order_change_id: Option<Uuid>,", "order change identity field"],
  ["operation: &'static str,", "route operation field"],
]) requireText(errorContext, value, label);

for (const [value, label] of [
  ["tenant_state: &'static str,", "tenant shape field"],
  ["actor_state: &'static str,", "actor shape field"],
  ["checkout_operation_state: &'static str,", "checkout operation shape field"],
  ["reservation_state: &'static str,", "reservation shape field"],
  ["payment_collection_state: &'static str,", "payment collection shape field"],
  ["payment_state: &'static str,", "payment shape field"],
  ["refund_state: &'static str,", "refund shape field"],
  ["order_state: &'static str,", "order shape field"],
  ["order_return_state: &'static str,", "order return shape field"],
  ["order_change_state: &'static str,", "order change shape field"],
  ["operation: &'static str,", "operation shape field"],
]) requireText(diagnosticContextType, value, label);

for (const [value, label] of [
  ["tenant_state: uuid_shape(context.tenant_id),", "tenant shape projection"],
  ["actor_state: uuid_shape(context.actor_id),", "actor shape projection"],
  [
    "checkout_operation_state: optional_uuid_shape(context.checkout_operation_id),",
    "checkout operation shape projection",
  ],
  [
    "reservation_state: optional_uuid_shape(context.reservation_id),",
    "reservation shape projection",
  ],
  [
    "payment_collection_state: optional_uuid_shape(context.payment_collection_id),",
    "payment collection shape projection",
  ],
  ["payment_state: optional_uuid_shape(context.payment_id),", "payment shape projection"],
  ["refund_state: optional_uuid_shape(context.refund_id),", "refund shape projection"],
  ["order_state: optional_uuid_shape(context.order_id),", "order shape projection"],
  [
    "order_return_state: optional_uuid_shape(context.order_return_id),",
    "order return shape projection",
  ],
  [
    "order_change_state: optional_uuid_shape(context.order_change_id),",
    "order change shape projection",
  ],
  ["operation: context.operation,", "operation label projection"],
]) requireText(diagnosticContext, value, label);

for (const [value, label] of [
  ['if value.is_nil() { "nil" } else { "non_nil" }', "nil/non-nil shape"],
  ['None => "absent",', "absent shape"],
  ['Some(value) if value.is_nil() => "present_nil",', "present-nil shape"],
  ['Some(_) => "present_non_nil",', "present-non-nil shape"],
]) requireText(shapeHelpers, value, label);

for (const [value, label] of [
  ["AdminErrorPolicy,", "shared admin policy type import"],
  ["payment_error_policy,", "shared payment policy import"],
  ["payment_orchestration_error_policy,", "shared payment orchestration policy import"],
  ["order_error_policy,", "shared order policy import"],
]) requireText(controllerImports, value, label);

for (const [value, label] of [
  ["use rustok_order::error::OrderError;", "typed order error import"],
  ["use rustok_payment::error::PaymentError;", "typed payment error import"],
  ["use sea_orm::DbErr;", "typed sweep database import"],
  ["CheckoutCompensationError,", "typed compensation error import"],
  ["CheckoutInventoryReservationError,", "typed reservation error import"],
  ["CheckoutOperationError,", "typed operation error import"],
  ["CheckoutReconciliationError,", "typed reconciliation error import"],
]) requireText(controller, value, label);

// The policy set is shared; the controller must not regrow a private copy per owner.
for (const value of [
  "fn payment_error_policy(",
  "fn reserved_refund_error_policy(",
  "fn order_error_policy(",
  "fn payment_orchestration_error_policy(",
]) forbidText(controller, value, "duplicated per-owner policy in the controller");

// --- Bounded diagnostics ------------------------------------------------------------------------

for (const [value, label] of [
  ["boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,", "route entry boundary"],
  ["operation,", "route entry operation label"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "route entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "route entry actor shape"],
  [
    "checkout_operation_id_present = facts.checkout_operation_id.is_some(),",
    "route entry operation presence",
  ],
  [
    "checkout_operation_id_non_nil = facts",
    "route entry operation non-nil shape",
  ],
  ["status_filter_present = facts.status_filter_present,", "route entry status filter shape"],
  ["limit_present = facts.limit_present,", "route entry limit shape"],
  [
    'reconciliation_action = facts.reconciliation_action.unwrap_or("none"),',
    "route entry action label",
  ],
]) requireText(routeEntryLogger, value, label);

for (const [value, label] of [
  ["owner = ADMIN_CHECKOUT_OPERATION_OWNER,", "recorded owner"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "recorded tenant shape"],
  ["operator_id_non_nil = !operator_id.is_nil(),", "recorded operator shape"],
  ["checkout_operation_id_non_nil = !record.checkout_operation_id.is_nil(),", "recorded operation shape"],
  ["reconciliation_action = record.action.as_str(),", "recorded action label"],
  ["result_status = record.result_status.as_str(),", "recorded result status label"],
  ["boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,", "recorded boundary"],
]) requireText(recordedLogger, value, label);

for (const [value, label] of [
  ["owner = ADMIN_CHECKOUT_OPERATION_OWNER,", "owner log"],
  ["source_owner,", "source owner log"],
  ["tenant_state = context.tenant_state,", "tenant shape log"],
  ["actor_state = context.actor_state,", "actor shape log"],
  ["checkout_operation_state = context.checkout_operation_state,", "operation shape log"],
  ["reservation_state = context.reservation_state,", "reservation shape log"],
  ["payment_collection_state = context.payment_collection_state,", "payment collection shape log"],
  ["payment_state = context.payment_state,", "payment shape log"],
  ["refund_state = context.refund_state,", "refund shape log"],
  ["order_state = context.order_state,", "order shape log"],
  ["order_return_state = context.order_return_state,", "order return shape log"],
  ["order_change_state = context.order_change_state,", "order change shape log"],
  ["operation = context.operation,", "route operation log"],
  ["error_kind,", "error-kind log"],
  ["public_code = code,", "public-code log"],
  ["status = %status,", "status log"],
  ["boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,", "boundary log"],
  ['"{log_message}"', "static log message"],
  ["HttpError::new(status, code, message)", "single static envelope constructor"],
]) requireText(httpErrorLogger, value, label);

forbidRawContextLogging(failures, controller, "admin checkout operation controller");
forbidRawContextLogging(failures, adminMod, "admin commerce mappers");

for (const value of [
  "error = ?error",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
  "reservation_id = ?context.reservation_id",
  "payment_collection_id = ?context.payment_collection_id",
  "operation = %context.operation",
  "admin_checkout_operation_http_error<E>(",
]) forbidText(controller, value, "retired raw diagnostic field");

// --- Route contracts ----------------------------------------------------------------------------

requireCount(
  controller,
  /log_admin_checkout_operation_route_entry\(\n        "/g,
  6,
  "six bounded route entry lines",
);
requireCount(
  controller,
  /map_(?:operation|compensation|sweep|reconciliation)_error\(\s+AdminCheckoutOperationErrorContext::new\(/g,
  6,
  "six context-aware mapper callsites",
);

const routes = [
  {
    label: "show checkout operation route",
    block: showRoute,
    permission: "[Permission::ORDERS_READ]",
    identity: "tenant.id, auth.user_id, Some(id),",
    operation: '"show_checkout_operation"',
    serviceCall: ".get(tenant.id, id)",
    mapper: "map_operation_error(",
    addressable: true,
  },
  {
    label: "compensate checkout operation route",
    block: compensateRoute,
    permission: "[Permission::ORDERS_MANAGE]",
    identity: "tenant.id, auth.user_id, Some(id),",
    operation: '"compensate_checkout_operation"',
    serviceCall: ".compensate(",
    mapper: "map_compensation_error(",
    addressable: true,
  },
  {
    label: "list checkout operations route",
    block: listRoute,
    permission: "[Permission::ORDERS_READ]",
    identity: "tenant.id, auth.user_id, None,",
    operation: '"list_checkout_operations"',
    serviceCall: ".list_by_status(",
    mapper: "map_operation_error(",
    addressable: false,
  },
  {
    label: "checkout reconciliation action route",
    block: reconciliationActionRoute,
    permission: "action.required_permissions()",
    identity: "tenant.id, auth.user_id, Some(id),",
    operation: '"execute_checkout_reconciliation_action"',
    serviceCall: ".execute(",
    mapper: "map_reconciliation_error(",
    addressable: true,
  },
  {
    label: "checkout reconciliation action list route",
    block: listActionsRoute,
    permission: "[Permission::ORDERS_READ]",
    identity: "tenant.id, auth.user_id, Some(id),",
    operation: '"list_checkout_reconciliation_actions"',
    serviceCall: ".list_actions(",
    mapper: "map_reconciliation_error(",
    addressable: true,
  },
  {
    label: "checkout compensation sweep route",
    block: sweepRoute,
    permission: "[Permission::ORDERS_MANAGE]",
    identity: "tenant.id, auth.user_id, None,",
    operation: '"sweep_checkout_compensations"',
    serviceCall: ".run(",
    mapper: "map_sweep_error(",
    addressable: false,
  },
];

for (const route of routes) {
  const { block, label } = route;
  requireText(block, "ensure_permissions(", `${label} permission gate`);
  requireText(block, route.permission, `${label} permission`);
  requireText(block, '"commerce_invalid_tenant"', `${label} tenant guard`);
  requireText(block, '"Tenant ID must not be nil"', `${label} tenant guard message`);
  requireText(block, route.operation, `${label} operation label`);
  requireText(block, "log_admin_checkout_operation_route_entry(", `${label} entry line`);
  requireText(block, ".map_err(|error| {", `${label} typed mapping closure`);
  requireText(block, "AdminCheckoutOperationErrorContext::new(", `${label} error context`);
  requireTextNormalized(block, route.identity, `${label} truthful identity`);
  requireText(block, route.serviceCall, `${label} service contract`);
  requireText(block, route.mapper, `${label} mapper handoff`);
  if (route.addressable) {
    requireText(block, '"commerce_invalid_id"', `${label} id guard`);
    requireText(block, '"Checkout operation ID must not be nil"', `${label} id guard message`);
    requireOrdered(
      block,
      ["if tenant.id.is_nil()", "if id.is_nil()", "log_admin_checkout_operation_route_entry("],
      `${label} guards before entry line`,
    );
  } else {
    requireOrdered(
      block,
      ["if tenant.id.is_nil()", "log_admin_checkout_operation_route_entry("],
      `${label} tenant guard before entry line`,
    );
  }
  requireOrdered(
    block,
    ["log_admin_checkout_operation_route_entry(", route.mapper, "AdminCheckoutOperationErrorContext::new("],
    `${label} entry line before mapper handoff`,
  );
}

requireText(
  reconciliationActionRoute,
  "let idempotency_key = require_idempotency_key(&headers)?;",
  "reconciliation idempotency key requirement",
);
requireText(
  reconciliationActionRoute,
  "log_admin_checkout_reconciliation_recorded(tenant.id, auth.user_id, &record);",
  "reconciliation recorded line",
);
requireText(
  reconciliationActionRoute,
  "Ok((StatusCode::CREATED, Json(map_reconciliation_action(record))))",
  "reconciliation created response",
);

for (const [value, label] of [
  [
    ".with_payment_provider_registry(runtime.payment_provider_registry())",
    "provider registry forwarding",
  ],
  [
    "rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone())",
    "inventory identity port forwarding",
  ],
  ["in_process_cart_checkout_port(runtime.db_clone())", "cart checkout port forwarding"],
  ['format!(\n                "admin-checkout-compensation:{}:{}"', "compensation lease owner"],
  ['format!("admin:{}", auth.user_id)', "sweep worker identity"],
  ["input.limit", "sweep limit forwarding"],
  ["Ok(Json(map_operation(operation)))", "operation response contract"],
  ["scanned: report.scanned", "sweep scanned response"],
  ["failures: report", "sweep failure response"],
]) requireText(controller, value, label);

// --- Checkout operation policy ------------------------------------------------------------------

const operationVariants = enumVariants(
  operationErrors,
  "CheckoutOperationError",
  "checkout operation error enum",
);
if (operationVariants.length !== 5) {
  failures.push(
    `checkout operation error enum: expected 5 variants, found ${operationVariants.length}`,
  );
}
requireEveryVariant(
  operationVariants,
  operationPolicy,
  "CheckoutOperationError",
  operationPolicy,
  "checkout operation policy",
);
forbidText(operationPolicy, "        _ =>", "wildcard in the checkout operation policy");

for (const [value, label] of [
  ["StatusCode::NOT_FOUND", "operation not-found status"],
  ["StatusCode::CONFLICT", "operation conflict status"],
  ["StatusCode::BAD_REQUEST", "operation validation status"],
  ["StatusCode::INTERNAL_SERVER_ERROR", "operation fail-closed status"],
  ['"checkout_operation_not_found"', "operation not-found code"],
  ['"checkout_operation_conflict"', "operation conflict code"],
  ['"checkout_operation_invalid"', "operation validation code"],
  ['"internal_error"', "operation internal code"],
  ['"checkout_operation_event_failed"', "operation event code"],
  ['"Checkout operation not found"', "static operation not-found message"],
  [
    '"Checkout operation conflicts with the current state"',
    "static operation conflict message",
  ],
  ['"Checkout operation request is invalid"', "static operation validation message"],
  ['"Checkout operation storage is unavailable"', "static operation storage message"],
  ['"Checkout operation event publication failed"', "static operation event message"],
]) requireText(operationPolicy, value, label);

requireText(
  operationMapper,
  "adopt_operation_error_identity(&mut context, &error);",
  "operation identity adoption",
);
requireText(operationMapper, '"rustok_commerce.checkout_operation"', "operation source owner");

// --- Shared admin policies ----------------------------------------------------------------------

const paymentVariants = enumVariants(paymentErrors, "PaymentError", "payment error enum");
if (paymentVariants.length !== 11) {
  failures.push(`payment error enum: expected 11 variants, found ${paymentVariants.length}`);
}
requireEveryVariant(
  paymentVariants,
  paymentPolicy,
  "PaymentError",
  paymentPolicy,
  "shared admin payment policy",
);
forbidText(paymentPolicy, "        _ =>", "wildcard in the shared payment policy");

for (const [value, label] of [
  ['"commerce_admin_not_found"', "payment not-found code"],
  ['"Commerce resource not found"', "payment not-found message"],
  ['"commerce_admin_payment_invalid"', "payment invalid code"],
  ['"commerce_admin_payment_state_conflict"', "payment conflict code"],
  ['"commerce_admin_payment_provider_unavailable"', "payment provider unavailable code"],
  ['"commerce_admin_payment_provider_invalid_response"', "payment provider response code"],
  ['"commerce_admin_payment_reconciliation_required"', "payment reconciliation code"],
  ['"commerce_admin_payment_provider_not_configured"', "payment provider configuration code"],
  ['"commerce_admin_payment_storage_unavailable"', "payment storage code"],
  ["StatusCode::BAD_GATEWAY", "payment invalid-response status"],
]) requireText(paymentPolicy, value, label);

for (const [value, label] of [
  ["PaymentError::ProviderOutcomeUnknown { .. }", "reserved unknown-outcome variant"],
  ["PaymentError::ProviderInvalidResponse { .. }", "reserved invalid-response variant"],
  ["PaymentError::ProviderUnavailable { .. }", "reserved unavailable variant"],
  ['"commerce_admin_refund_reconciliation_required"', "reserved refund reconciliation code"],
  [
    '"Refund remains reserved while the provider outcome is reconciled"',
    "reserved refund reconciliation message",
  ],
  ['"commerce_admin_refund_provider_unavailable"', "reserved refund retry code"],
  [
    '"Refund remains reserved and the provider operation may be retried safely"',
    "reserved refund retry message",
  ],
  ["other => payment_error_policy(other)", "reserved refund fallback to the payment policy"],
]) requireText(reservedRefundPolicy, value, label);

const orchestrationVariants = enumVariants(
  paymentOrchestration,
  "PaymentOrchestrationError",
  "payment orchestration error enum",
);
if (orchestrationVariants.length !== 3) {
  failures.push(
    `payment orchestration error enum: expected 3 variants, found ${orchestrationVariants.length}`,
  );
}
requireEveryVariant(
  orchestrationVariants,
  orchestrationPolicy,
  "PaymentOrchestrationError",
  orchestrationPolicy,
  "shared admin payment orchestration policy",
);
forbidText(orchestrationPolicy, "        _ =>", "wildcard in the orchestration policy");

for (const [value, label] of [
  ["(payment_error_policy(source), \"rustok_payment\")", "orchestration payment policy handoff"],
  ["reserved_refund_error_policy(source),", "orchestration reserved refund handoff"],
  ['"rustok_payment.refund_reservation"', "reserved refund source owner"],
]) requireText(orchestrationPolicy, value, label);

const orderVariants = enumVariants(orderErrors, "OrderError", "order error enum");
requireEveryVariant(
  orderVariants,
  orderPolicy,
  "OrderError",
  orderPolicy,
  "shared admin order policy",
);
forbidText(orderPolicy, "        _ =>", "wildcard in the shared order policy");

for (const [value, label] of [
  ['"commerce_admin_order_invalid"', "order invalid code"],
  ['"commerce_admin_order_state_conflict"', "order conflict code"],
  ['"commerce_admin_order_storage_unavailable"', "order storage code"],
  ['"commerce_admin_order_failed"', "order fail-closed code"],
  ['"Order request is invalid"', "static order invalid message"],
  ['"Commerce resource not found"', "static order not-found message"],
  ['"Order operation conflicts with the current state"', "static order conflict message"],
  ['"Order storage is temporarily unavailable"', "static order storage message"],
  ['"Order operation could not be completed safely"', "static order fail-closed message"],
]) requireText(orderPolicy, value, label);

requireText(
  orderMapper,
  'admin_public_error("rustok_order", error_kind, status, code, message)',
  "order mapper keeps the shared admin public error helper",
);

// --- Compensation mapper ------------------------------------------------------------------------

const compensationVariants = enumVariants(
  compensationErrors,
  "CheckoutCompensationError",
  "checkout compensation error enum",
);
for (const required of [
  "Operation",
  "ReservationJournal",
  "Payment",
  "PaymentOrchestration",
  "Order",
  "ManualReconciliation",
  "Conflict",
  "Boundary",
  "CompensationAndJournal",
]) {
  if (!compensationVariants.includes(required)) {
    failures.push(`checkout compensation error enum: missing variant ${required}`);
  }
}
requireEveryVariant(
  compensationVariants,
  compensationMapper,
  "CheckoutCompensationError",
  compensationMapper,
  "checkout compensation mapper",
);
forbidText(compensationMapper, "        _ =>", "wildcard in the compensation mapper");

for (const [value, label] of [
  ["adopt_operation_error_identity(&mut context, source)", "operation identity adoption"],
  ["adopt_reservation_error_identity(&mut context, source)", "reservation identity adoption"],
  ["adopt_payment_error_identity(&mut context, source)", "payment identity adoption"],
  ["adopt_order_error_identity(&mut context, source)", "order identity adoption"],
  ["checkout_operation_error_policy(source)", "operation policy handoff"],
  ['"rustok_commerce.checkout_operation"', "operation source owner"],
  ['"rustok_commerce.checkout_inventory_reservation"', "reservation source owner"],
  ["(payment_error_policy(source), \"rustok_payment\")", "payment policy handoff"],
  ["payment_orchestration_error_policy(source)", "payment orchestration policy handoff"],
  ["(order_error_policy(source), \"rustok_order\")", "order policy handoff"],
  ['"checkout_reconciliation_required"', "manual reconciliation code"],
  ['"Checkout requires manual reconciliation"', "static manual reconciliation message"],
  ['"checkout_compensation_conflict"', "compensation conflict code"],
  [
    '"Checkout compensation cannot proceed from the current state"',
    "static compensation conflict message",
  ],
  ['"checkout_compensation_pending"', "retryable compensation code"],
  ['"Checkout compensation will be retried"', "static retryable compensation message"],
  ['"retryable_boundary"', "retryable boundary kind"],
  ['"Checkout compensation is unavailable"', "fail-closed compensation message"],
  ['"compensation_failed"', "compensation failure kind"],
  ['"rustok_commerce.checkout_compensation"', "compensation source owner"],
  ["compensation_boundary_owner(stage)", "boundary owner selection"],
  [
    '"commerce admin checkout compensation failed"',
    "compensation diagnostic message",
  ],
]) requireText(compensationMapper, value, label);

requireCount(
  compensationMapper,
  /CheckoutCompensationError::Boundary \{/g,
  2,
  "retryable and terminal boundary branches",
);

for (const [value, label] of [
  ['"compensate_payment" => "rustok_payment",', "payment boundary owner"],
  ['"compensate_order" => "rustok_order",', "order boundary owner"],
  ['"release_inventory" => "rustok_inventory",', "inventory boundary owner"],
  ['"read_cart" | "release_cart" => "rustok_cart",', "cart boundary owner"],
  ['_ => "rustok_commerce.checkout_compensation",', "fallback boundary owner"],
]) requireText(boundaryOwner, value, label);

for (const [value, label] of [
  ["fn adopt_operation_error_identity(", "operation identity adoption helper"],
  [
    "if let CheckoutOperationError::NotFound(id) = error { context.checkout_operation_id = Some(*id);",
    "operation identity projection",
  ],
  ["fn adopt_reservation_error_identity(", "reservation identity adoption helper"],
  [
    "if let CheckoutInventoryReservationError::NotFound(id) = error { context.reservation_id = Some(*id);",
    "reservation identity projection",
  ],
  ["fn adopt_payment_error_identity(", "payment identity adoption helper"],
  ["PaymentError::PaymentCollectionNotFound(id) => context.payment_collection_id = Some(*id)",
   "payment collection identity projection"],
  ["PaymentError::PaymentNotFound(id) => context.payment_id = Some(*id)",
   "payment identity projection"],
  ["PaymentError::RefundNotFound(id) => context.refund_id = Some(*id)",
   "refund identity projection"],
  ["fn adopt_order_error_identity(", "order identity adoption helper"],
  ["OrderError::OrderNotFound(id) => context.order_id = Some(*id)", "order identity projection"],
  ["OrderError::OrderReturnNotFound(id) => context.order_return_id = Some(*id)",
   "order return identity projection"],
  ["OrderError::OrderChangeNotFound(id) => context.order_change_id = Some(*id)",
   "order change identity projection"],
]) requireTextNormalized(identityAdoption, value, label);

// --- Reconciliation policy and mapper -----------------------------------------------------------

requireText(
  reconciliationErrorMapper,
  "let (policy, source_owner) = reconciliation_error_policy(&error);",
  "reconciliation policy selection",
);
requireText(
  reconciliationErrorMapper,
  '"commerce admin checkout reconciliation action failed"',
  "reconciliation diagnostic message",
);

for (const [value, label] of [
  ['"checkout_reconciliation_invalid_request"', "reconciliation invalid code"],
  ['"checkout_operation_not_found"', "reconciliation not-found code"],
  ['"checkout_reconciliation_idempotency_conflict"', "reconciliation idempotency code"],
  ['"checkout_reconciliation_conflict"', "reconciliation conflict code"],
  ['"checkout_reconciliation_pending"', "reconciliation retryable code"],
  ['"Checkout reconciliation will be retried"', "static reconciliation retryable message"],
  ['"checkout_reconciliation_payment_failed"', "reconciliation payment failure code"],
  ['"Payment owner rejected the reconciliation step"', "static payment rejection message"],
  [
    '"Checkout reconciliation requires operator follow-up"',
    "static close-after-money-moved message",
  ],
  ["checkout_operation_error_policy(source)", "reconciliation operation policy handoff"],
  ['"rustok_payment.admin_refund_command"', "payment owner label"],
  ['"rustok_commerce.checkout_reconciliation"', "reconciliation owner label"],
  ['"rustok_commerce.checkout_operation"', "operation owner label"],
]) requireText(reconciliationPolicy, value, label);

for (const [value, label] of [
  ["retryable: true,", "retryable payment owner branch"],
  ["CheckoutReconciliationError::CloseAfterMoneyMoved { .. }", "close-after-money-moved branch"],
  ["CheckoutReconciliationError::Operation(source)", "operation branch"],
  ["CheckoutReconciliationError::Database(_)", "database branch"],
]) requireText(reconciliationPolicy, value, label);

// --- Sweep mapper and sweep owner codes ---------------------------------------------------------

for (const [value, label] of [
  ["_error: DbErr)", "typed sweep database cause"],
  ['"rustok_commerce.checkout_compensation_sweep"', "sweep source owner"],
  ['"Checkout compensation storage is unavailable"', "static sweep storage message"],
  ['"database"', "sweep database kind"],
  ['"commerce admin checkout compensation sweep failed"', "sweep diagnostic message"],
  [".into()", "unused sweep cause marker"],
].slice(0, 5)) requireText(sweepMapper, value, label);

for (const [value, label] of [
  ['"checkout.compensation_payment_failed"', "safe payment sweep code"],
  ['"checkout.compensation_order_failed"', "safe order sweep code"],
  ['"checkout.compensation_boundary_failed"', "safe boundary sweep code"],
  ['"checkout.compensation_inventory_failed"', "safe inventory sweep code"],
  ['"checkout.compensation_manual_reconciliation"', "safe manual reconciliation sweep code"],
  ['"checkout.compensation_failed"', "safe generic sweep code"],
]) requireText(sweep, value, label);

// --- Retired conversions ------------------------------------------------------------------------

for (const [value, label] of [
  [".map_err(map_operation_error)?;", "untyped operation mapper"],
  [".map_err(map_compensation_error)?;", "untyped compensation mapper"],
  [".map_err(|_| HttpError::internal(", "context-free internal error"],
  ['HttpError::new(StatusCode::CONFLICT, "checkout_operation_conflict", message)', "owner text leak"],
  ['HttpError::bad_request("checkout_operation_invalid", message)', "owner text leak"],
  ["error.to_string()", "owner text leak"],
  ["err.to_string()", "owner text leak"],
  ["other.to_string()", "owner text leak"],
]) forbidText(controller, value, label);

if (failures.length > 0) {
  console.error("Commerce admin checkout-operation error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin checkout operations keep typed owner causes, shared static policies, and bounded diagnostics",
);
