#!/usr/bin/env node

// Admin order-return orchestration error context.
//
// Two routes orchestrate a return across owners:
//
//   POST /admin/orders/{id}/returns/decision -> create_order_return_decision
//   POST /admin/returns/{id}/complete        -> complete_order_return
//
// The decision route talks to the Order owner through the host-selected post-order command port and
// to the Payment owner through the admin read port, then falls back to the post-order orchestration
// error type. The completion route runs the return-completion orchestration service, which reports
// `PostOrderOrchestrationError`. Every one of those failures must land on a static Commerce envelope
// selected by policy, with the owner identity adopted into the context and only shapes in the log.
//
// The verifier used to describe the earlier shape of this controller: local copies of the order,
// payment and reserved-refund policies (`admin_order_error_policy`, `admin_payment_error_policy`,
// `admin_reserved_refund_error_policy`), `error = ?error`, raw `tenant_id = %context.tenant_id` /
// `actor_id = %context.actor_id` / `order_id = ?context.order_id` fields, and a deferred
// `PaymentService::new(self.db.clone())` compatibility path. The policies now live once in
// `controllers/admin/mod.rs`, the log line records `*_shape` values, and the guard is written against
// the owner-port orchestration that actually ships.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), "utf8");

const controller = read("crates/modules/rustok-commerce/src/controllers/admin/returns.rs");
const adminMod = read("crates/modules/rustok-commerce/src/controllers/admin/mod.rs");
const postOrder = read("crates/modules/rustok-commerce/src/services/post_order.rs");
const returnDecision = read(
  "crates/modules/rustok-commerce/src/services/return_decision_owner_orchestration.rs",
);
const paymentOrchestration = read(
  "crates/modules/rustok-commerce/src/services/payment_orchestration.rs",
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
const requireCountNormalized = (content, value, expected, label) => {
  const needle = normalize(value);
  const haystack = normalize(content);
  let actual = 0;
  let index = haystack.indexOf(needle);
  while (index >= 0) {
    actual += 1;
    index = haystack.indexOf(needle, index + needle.length);
  }
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
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
const requireEveryVariant = (variants, content, enumName, label) => {
  for (const variant of variants) {
    requirePattern(content, new RegExp(`${enumName}::${variant}\\b`), `${label} handles ${variant}`);
  }
};

const imports = between(controller, "use super::{", "use crate::{", "order-return imports");
const constants = between(
  controller,
  "const ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER:",
  "type AdminOrderReturnHttpPolicy",
  "order-return constants",
);
const policyAlias = between(
  controller,
  "type AdminOrderReturnHttpPolicy",
  "struct AdminOrderReturnOrchestrationErrorContext",
  "order-return policy alias",
);
const contextType = between(
  controller,
  "struct AdminOrderReturnOrchestrationErrorContext {",
  "impl AdminOrderReturnOrchestrationErrorContext {",
  "order-return error context type",
);
const contextConstructor = between(
  controller,
  "impl AdminOrderReturnOrchestrationErrorContext {",
  "fn log_admin_order_return_entry(",
  "order-return error context constructor",
);
const entryLogger = between(
  controller,
  "fn log_admin_order_return_entry(",
  "fn require_idempotency_key(",
  "order-return entry logger",
);
const portContextBuilder = between(
  controller,
  "fn admin_return_decision_order_context(",
  "fn optional_uuid_shape(",
  "return decision port context builder",
);
const orderPortPolicy = between(
  controller,
  "fn admin_order_port_error_policy(",
  "fn admin_payment_port_error_policy(",
  "order owner port policy",
);
const paymentPortPolicy = between(
  controller,
  "fn admin_payment_port_error_policy(",
  "fn map_admin_return_decision_order_port_error(",
  "payment owner port policy",
);
const orderPortMapper = between(
  controller,
  "fn map_admin_return_decision_order_port_error(",
  "fn map_admin_return_decision_payment_port_error(",
  "return decision order port mapper",
);
const paymentPortMapper = between(
  controller,
  "fn map_admin_return_decision_payment_port_error(",
  "fn admin_owner_port_policy(",
  "return decision payment port mapper",
);
const ownerPortDispatch = between(
  controller,
  "fn admin_owner_port_policy(",
  "fn map_admin_order_return_orchestration_error(",
  "owner port dispatch",
);
const orchestrationMapper = between(
  controller,
  "fn map_admin_order_return_orchestration_error(",
  "#[utoipa::path(",
  "order-return orchestration mapper",
);
const decisionRoute = between(
  controller,
  "pub async fn create_order_return_decision(",
  "pub async fn complete_order_return(",
  "return decision route",
);
const completionRoute = from(controller, "pub async fn complete_order_return(", "return completion route");

// --- Wiring ------------------------------------------------------------------------------------

for (const [value, label] of [
  ["AdminErrorPolicy,", "shared policy type import"],
  ["order_error_policy,", "shared order policy import"],
  ["payment_error_policy,", "shared payment policy import"],
  ["reserved_refund_error_policy,", "shared reserved refund policy import"],
  ["common::ensure_permissions", "permission helper import"],
]) requireText(imports, value, label);

for (const [value, label] of [
  [
    'const ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER: &str =\n    "rustok_commerce.admin_order_return_orchestration";',
    "orchestration owner",
  ],
  [
    'const ADMIN_ORDER_RETURN_BOUNDARY: &str = "commerce_admin_order_return_http";',
    "orchestration boundary",
  ],
]) requireText(constants, value, label);

requireText(
  policyAlias,
  "type AdminOrderReturnHttpPolicy = AdminErrorPolicy;",
  "order-return policy alias reuses the shared admin policy shape",
);

for (const forbidden of [
  "fn admin_payment_error_policy(",
  "fn admin_order_error_policy(",
  "fn admin_reserved_refund_error_policy(",
]) forbidText(controller, forbidden, "duplicated per-owner policy in the return controller");

for (const [value, label] of [
  ["tenant_id: Uuid,", "tenant field"],
  ["actor_id: Uuid,", "actor field"],
  ["order_id: Option<Uuid>,", "order identity field"],
  ["return_id: Option<Uuid>,", "return identity field"],
  ["refund_id: Option<Uuid>,", "refund identity field"],
  ["operation: &'static str,", "operation field"],
  ["refund_id: None,", "refund identity default"],
]) {
  const content = label === "refund identity default" ? contextConstructor : contextType;
  requireText(content, value, label);
}

requireText(
  contextConstructor,
  "fn new(\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        order_id: Option<Uuid>,\n        return_id: Option<Uuid>,\n        operation: &'static str,\n    ) -> Self {",
  "order-return context constructor signature",
);

// --- Bounded entry line ------------------------------------------------------------------------

for (const [value, label] of [
  ["operation: &'static str,", "entry operation"],
  ["resource_kind: &'static str,", "entry resource kind"],
  ["decision_action_length: usize,", "bounded decision action length"],
  ["decision_moves_money: bool,", "bounded decision classification"],
  ["tenant_id_non_nil = !tenant_id.is_nil(),", "entry tenant shape"],
  ["actor_id_non_nil = !actor_id.is_nil(),", "entry actor shape"],
  ["resource_id_non_nil = !resource_id.is_nil(),", "entry resource shape"],
  ["decision_action_length,", "decision action length field"],
  ["decision_moves_money,", "decision classification field"],
  ["boundary = ADMIN_ORDER_RETURN_BOUNDARY,", "entry boundary"],
  ['"commerce admin order return entered with bounded diagnostics"', "entry message"],
]) requireText(entryLogger, value, label);

requireCount(controller, /log_admin_order_return_entry\(\n        "/g, 2, "two bounded entry lines");
requireCount(controller, /tracing::debug!\(/g, 1, "one debug line");
requireCount(controller, /tracing::info!\(/g, 0, "no info line");
requireCount(controller, /tracing::error!\(/g, 3, "three error lines");
requireCount(controller, /tracing::warn!\(/g, 0, "no warn line");

forbidRawContextLogging(failures, controller, "admin order-return controller");
forbidRawContextLogging(failures, adminMod, "admin commerce shared mappers");
for (const forbidden of [
  "error = ?error",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "order_id = ?context.order_id",
  "return_id = ?context.return_id",
  "refund_id = ?context.refund_id",
  "operation = %context.operation",
  "PaymentService::new(self.db.clone())",
]) forbidText(controller, forbidden, "retired raw or deferred path");

// --- Port contexts and port policies -----------------------------------------------------------

for (const [value, label] of [
  ["PortContext::new(", "port context constructor"],
  ["PortActor::user(auth.user_id.to_string())", "actor projection"],
  ["tenant.id.to_string()", "tenant projection"],
  ['format!("commerce-admin-return-decision:{order_id}")', "correlation key"],
  [".with_idempotency_key(idempotency_key)", "idempotency key forwarding"],
  [".with_deadline(std::time::Duration::from_secs(2))", "port deadline"],
  ["context = context.with_channel(channel);", "channel forwarding"],
]) requireText(portContextBuilder, value, label);

const portErrorKinds = enumVariants(portErrors, "PortErrorKind", "port error kind enum");
if (portErrorKinds.length !== 7) {
  failures.push(`port error kind enum: expected 7 variants, found ${portErrorKinds.length}`);
}
for (const [block, label] of [
  [orderPortPolicy, "order owner port policy"],
  [paymentPortPolicy, "payment owner port policy"],
]) {
  requireEveryVariant(portErrorKinds, block, "PortErrorKind", label);
}

for (const [value, label] of [
  ['"commerce_admin_order_invalid"', "order validation code"],
  ['"Order request is invalid"', "order validation message"],
  ['"commerce_admin_not_found"', "order not-found code"],
  ['"Commerce resource not found"', "order not-found message"],
  ['"commerce_admin_order_state_conflict"', "order conflict code"],
  ['"Order operation conflicts with the current state"', "order conflict message"],
  ['"commerce_permission_denied"', "order forbidden code"],
  ['"Permission denied"', "order forbidden message"],
  ['"commerce_admin_order_storage_unavailable"', "order storage code"],
  ['"Order storage is temporarily unavailable"', "order storage message"],
  ['"commerce_admin_order_failed"', "order fail-closed code"],
  ['"Order operation could not be completed safely"', "order fail-closed message"],
]) requireText(orderPortPolicy, value, label);

for (const [value, label] of [
  ['"commerce_admin_payment_invalid"', "payment validation code"],
  ['"Payment request is invalid"', "payment validation message"],
  ['"commerce_admin_payment_state_conflict"', "payment conflict code"],
  ['"Payment operation conflicts with the current state"', "payment conflict message"],
  ['"commerce_admin_payment_storage_unavailable"', "payment storage code"],
  ['"Payment storage is temporarily unavailable"', "payment storage message"],
  ['"commerce_admin_payment_failed"', "payment fail-closed code"],
  ['"Payment state could not be read safely"', "payment fail-closed message"],
]) requireText(paymentPortPolicy, value, label);

for (const [mapper, label, owner, ownerOperation, consumerOperation] of [
  [
    orderPortMapper,
    "return decision order port mapper",
    'owner = "rustok_order.post_order_command",',
    null,
    'consumer_operation = "create_return_decision",',
  ],
  [
    paymentPortMapper,
    "return decision payment port mapper",
    'owner = "rustok_payment.admin_read",',
    'owner_operation = "list_payment_collection_projections",',
    'consumer_operation = "create_return_decision",',
  ],
]) {
  requireText(mapper, owner, `${label}: owner`);
  if (ownerOperation !== null) requireText(mapper, ownerOperation, `${label}: owner operation`);
  requireText(mapper, consumerOperation, `${label}: consumer operation`);
  for (const [value, field] of [
    ["correlation_id = %context.correlation_id,", "correlation join key"],
    ["tenant_id_non_nil = !tenant_id.is_nil(),", "tenant shape"],
    ["actor_id_non_nil = !actor_id.is_nil(),", "actor shape"],
    ["order_id_non_nil = !order_id.is_nil(),", "order shape"],
    ["owner_error_kind = ?error.kind,", "typed owner kind"],
    ["owner_code_length = error.code.chars().count(),", "bounded owner code"],
    ["retryable = error.retryable,", "retryability"],
    ["public_code = code,", "public code"],
    ["status = %status,", "status"],
    ["boundary = ADMIN_ORDER_RETURN_BOUNDARY,", "boundary"],
    ["HttpError::new(status, code, message)", "static envelope"],
  ]) requireText(mapper, value, `${label}: ${field}`);
}

for (const [value, label] of [
  ['"rustok_payment" => admin_payment_port_error_policy(error),', "payment owner dispatch"],
  ["_ => admin_order_port_error_policy(error),", "order owner fallback"],
]) requireText(ownerPortDispatch, value, label);

// --- Orchestration mapper ----------------------------------------------------------------------

const postOrderVariants = enumVariants(
  postOrder,
  "PostOrderOrchestrationError",
  "post-order orchestration error enum",
);
if (postOrderVariants.length !== 5) {
  failures.push(
    `post-order orchestration error enum: expected 5 variants, found ${postOrderVariants.length}`,
  );
}
requireEveryVariant(
  postOrderVariants,
  orchestrationMapper,
  "PostOrderOrchestrationError",
  "order-return orchestration mapper",
);

const paymentOrchestrationVariants = enumVariants(
  paymentOrchestration,
  "PaymentOrchestrationError",
  "payment orchestration error enum",
);
requireEveryVariant(
  paymentOrchestrationVariants,
  orchestrationMapper,
  "PaymentOrchestrationError",
  "order-return orchestration mapper",
);

for (const [value, label] of [
  ["order_error_policy(source)", "order policy handoff"],
  ["payment_error_policy(source)", "payment policy handoff"],
  ["reserved_refund_error_policy(source)", "reserved refund policy handoff"],
  ["admin_owner_port_policy(owner, error)", "owner port policy handoff"],
  ['"rustok_order"', "order source owner"],
  ['"rustok_payment"', "payment source owner"],
  ['"rustok_commerce"', "commerce source owner"],
  ["OrderError::OrderNotFound(id) => context.order_id = Some(*id),", "order identity adoption"],
  [
    "OrderError::OrderReturnNotFound(id) => context.return_id = Some(*id),",
    "return identity adoption",
  ],
  [
    "if let PaymentError::RefundNotFound(id) = source { context.refund_id = Some(*id);",
    "refund identity adoption",
  ],
  ["context.refund_id = Some(*refund_id);", "reserved refund identity adoption"],
  ['"commerce_admin_post_order_invalid"', "post-order validation code"],
  ['"Post-order request is invalid"', "post-order validation message"],
  ['"commerce admin order return orchestration failed with bounded diagnostics"', "mapper message"],
]) requireTextNormalized(orchestrationMapper, value, label);

for (const [value, label] of [
  ["let tenant_id_non_nil = !context.tenant_id.is_nil();", "tenant shape"],
  ["let actor_id_non_nil = !context.actor_id.is_nil();", "actor shape"],
  ["let order_id_shape = optional_uuid_shape(context.order_id);", "order shape"],
  ["let return_id_shape = optional_uuid_shape(context.return_id);", "return shape"],
  ["let refund_id_shape = optional_uuid_shape(context.refund_id);", "refund shape"],
  ["owner = ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER,", "orchestration owner field"],
  ["source_owner,", "source owner field"],
  ["tenant_id_non_nil,", "tenant shape field"],
  ["actor_id_non_nil,", "actor shape field"],
  ["order_id_shape,", "order shape field"],
  ["return_id_shape,", "return shape field"],
  ["refund_id_shape,", "refund shape field"],
  ["operation = context.operation,", "operation field"],
  ["error_kind,", "error kind field"],
  ["public_code = code,", "public code field"],
  ["status = %status,", "status field"],
  ["boundary = ADMIN_ORDER_RETURN_BOUNDARY,", "boundary field"],
  ["HttpError::new(status, code, message)", "static envelope"],
]) requireTextNormalized(orchestrationMapper, value, label);

for (const forbidden of ["format!(", ".to_string()", "error.message", "?error", "%error"]) {
  forbidText(orchestrationMapper, forbidden, "raw orchestration mapper payload");
}

// --- Route contracts ---------------------------------------------------------------------------

const returnDecisionVariants = enumVariants(
  returnDecision,
  "ReturnDecisionOwnerOrchestrationError",
  "return decision orchestration error enum",
);
if (returnDecisionVariants.length !== 3) {
  failures.push(
    `return decision orchestration error enum: expected 3 variants, found ${returnDecisionVariants.length}`,
  );
}

for (const [value, label] of [
  ["ensure_permissions(", "permission gate"],
  ["[Permission::ORDERS_UPDATE]", "order update permission"],
  ['"Permission denied: orders:update required"', "order update message"],
  [
    "let decision_moves_money = super::decision_requires_payments_update(",
    "payment permission classification",
  ],
  ["if decision_moves_money {", "conditional payment permission gate"],
  ["[Permission::PAYMENTS_UPDATE]", "payment update permission"],
  ['"Permission denied: payments:update required"', "payment update message"],
  ['"commerce_invalid_tenant"', "tenant guard"],
  ['"commerce_invalid_id"', "id guard"],
  ["log_admin_order_return_entry(", "bounded entry line"],
  ["require_idempotency_key(&headers)?", "idempotency key"],
  ["admin_return_decision_order_context(&tenant, &auth, &request_context, id, idempotency_key)",
   "port context construction"],
  ["ReturnDecisionOwnerOrchestrationService::new(", "owner orchestration service"],
  ["runtime.order_post_order_command_port(),", "host-selected Order command port"],
  ["runtime.payment_admin_read_port(),", "host-selected Payment read port"],
  [".with_order_read_port(runtime.order_read_port())", "Order read port forwarding"],
  [".with_payment_provider_registry(runtime.payment_provider_registry())",
   "payment provider registry forwarding"],
  [".create_return_decision(context.clone(), tenant.id, id, input)", "decision owner call"],
  [".map_err(|error| match error {", "typed error dispatch"],
  ["ReturnDecisionOwnerOrchestrationError::OrderCommand(error)", "order command branch"],
  ["ReturnDecisionOwnerOrchestrationError::PaymentRead(error)", "payment read branch"],
  ["ReturnDecisionOwnerOrchestrationError::PostOrder(error)", "post-order branch"],
  ["map_admin_return_decision_order_port_error(", "order port mapper handoff"],
  ["map_admin_return_decision_payment_port_error(", "payment port mapper handoff"],
  ["map_admin_order_return_orchestration_error(", "orchestration mapper handoff"],
  ['"create_return_decision",', "decision operation label"],
  ["Ok((StatusCode::CREATED, Json(decision)))", "decision created response"],
]) requireText(decisionRoute, value, label);

for (const variant of returnDecisionVariants) {
  requirePattern(
    decisionRoute,
    new RegExp(`ReturnDecisionOwnerOrchestrationError::${variant}\\b`),
    `decision route handles ReturnDecisionOwnerOrchestrationError::${variant}`,
  );
}

requireOrder(
  decisionRoute,
  [
    "[Permission::ORDERS_UPDATE]",
    "if decision_moves_money {",
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_return_entry(",
    "require_idempotency_key(&headers)?",
    ".create_return_decision(",
    ".map_err(|error| match error {",
  ],
  "decision route order",
);

function requireOrder(content, values, label) {
  requireOrdered(content, values, label);
}

for (const [value, label] of [
  ["ensure_permissions(", "permission gate"],
  ["[Permission::ORDERS_UPDATE]", "order update permission"],
  ["let refund_present = input.refund.is_some();", "refund presence classification"],
  ["if refund_present {", "conditional payment permission gate"],
  ["[Permission::PAYMENTS_UPDATE]", "payment update permission"],
  ['"commerce_invalid_tenant"', "tenant guard"],
  ['"commerce_invalid_id"', "id guard"],
  ["log_admin_order_return_entry(", "bounded entry line"],
  ['"complete_order_return",', "completion operation label"],
  ['"order_return",', "completion resource kind"],
  ["CompleteReturnResolutionInput {", "typed completion command"],
  ["CompleteReturnRefundInput {", "typed refund command"],
  ["CompleteReturnExchangeInput {", "typed exchange command"],
  ["CompleteReturnClaimInput {", "typed claim command"],
  ["runtime .return_completion_orchestration()", "host-selected completion orchestration"],
  [".complete_return(tenant.id, auth.user_id, id, command)", "completion owner call"],
  [".map_err(|error| {", "typed mapping closure"],
  ["AdminOrderReturnOrchestrationErrorContext::new(", "completion error context"],
  ['"complete_return",', "completion context operation"],
  ["Ok(Json(item))", "completion response"],
]) requireTextNormalized(completionRoute, value, label);

requireCountNormalized(
  completionRoute,
  'AdminOrderReturnOrchestrationErrorContext::new( tenant.id, auth.user_id, None, Some(id), "complete_return", ),',
  1,
  "completion truthful route identity",
);

requireOrdered(
  completionRoute,
  [
    "[Permission::ORDERS_UPDATE]",
    "if refund_present {",
    "if tenant.id.is_nil()",
    "if id.is_nil()",
    "log_admin_order_return_entry(",
    ".complete_return(",
    ".map_err(|error| {",
  ],
  "completion route order",
);

// --- Shared policies and retired conversions ---------------------------------------------------

for (const [value, label] of [
  ["pub(crate) fn order_error_policy(", "shared order policy"],
  ["pub(crate) fn payment_error_policy(", "shared payment policy"],
  ["pub(crate) fn reserved_refund_error_policy(", "shared reserved refund policy"],
  ["pub(crate) type AdminErrorPolicy", "shared admin policy type"],
]) requireText(adminMod, value, label);

for (const forbidden of [
  ".map_err(|_| HttpError::internal(",
  "err.to_string()",
  "error.to_string()",
  "error.message",
]) forbidText(controller, forbidden, "owner text leak");

if (failures.length > 0) {
  console.error("Commerce admin order-return orchestration error-context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order-return orchestration keeps owner-typed causes, shared static policies, and bounded diagnostics",
);
