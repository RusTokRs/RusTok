#!/usr/bin/env node

// Admin order-change owner-port orchestration error context.
//
// `apply_order_change` runs `OrderChangeOrchestrationService::from_order_ports`, which can fail in
// three places: the Order read port (`OrderRead`), the Order command port (`OrderCommand`), or the
// cross-owner post-order orchestration (`PostOrder`). The first two reuse the port mapper with the
// context they were handed; the third dispatches over `PostOrderOrchestrationError`, adopts the order,
// payment, and refund identity the owner error carries, and maps each variant through the shared
// admin policies.
//
// The verifier used to describe the retired in-process orchestration: a typed `context` plus
// `map_admin_order_change_orchestration_error(context, source)` handoff, raw
// `tenant_id = %context.tenant_id` / `order_id = ?context.order_id` fields, `error = ?error`, and a
// local reserved-refund policy. The controller now composes host-selected ports, the policies are
// shared from `controllers/admin/mod.rs`, and the log line records shapes. This guard pins that.

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
const orderChangeOrchestration = read(
  "crates/modules/rustok-commerce/src/services/order_change_orchestration.rs",
);
const postOrder = read("crates/modules/rustok-commerce/src/services/post_order.rs");
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

const contextType = between(
  controller,
  "struct AdminOrderChangeOrchestrationErrorContext {",
  "impl AdminOrderChangeOrchestrationErrorContext {",
  "order-change error context type",
);
const contextConstructor = between(
  controller,
  "impl AdminOrderChangeOrchestrationErrorContext {",
  "fn optional_uuid_shape(",
  "order-change error context constructor",
);
const identityAdoption = between(
  controller,
  "fn adopt_order_change_order_error_identity(",
  "fn map_admin_order_change_port_error(",
  "order-change identity adoption",
);
const orchestrationMapper = between(
  controller,
  "fn map_admin_order_change_orchestration_error(",
  "fn map_admin_order_change_apply_error(",
  "order-change orchestration mapper",
);
const applyMapper = between(
  controller,
  "fn map_admin_order_change_apply_error(",
  "#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]",
  "order-change apply error mapper",
);
const applyRoute = from(controller, "pub async fn apply_order_change(", "apply order change route");

// --- Error context -----------------------------------------------------------------------------

for (const [value, label] of [
  ["tenant_id: Uuid,", "tenant field"],
  ["actor_id: Uuid,", "actor field"],
  ["order_id: Option<Uuid>,", "order identity field"],
  ["order_change_id: Option<Uuid>,", "order-change identity field"],
  ["payment_collection_id: Option<Uuid>,", "payment collection identity field"],
  ["payment_id: Option<Uuid>,", "payment identity field"],
  ["refund_id: Option<Uuid>,", "refund identity field"],
  ["operation: &'static str,", "operation field"],
]) requireText(contextType, value, label);

for (const [value, label] of [
  ["order_change_id: Some(order_change_id),", "order-change identity default"],
  ["order_id: None,", "order identity default"],
  ["payment_collection_id: None,", "payment collection identity default"],
  ["refund_id: None,", "refund identity default"],
]) requireText(contextConstructor, value, label);

for (const [value, label] of [
  ["OrderError::OrderNotFound(id) => context.order_id = Some(*id),", "order identity adoption"],
  [
    "OrderError::OrderChangeNotFound(id) => context.order_change_id = Some(*id),",
    "order-change identity adoption",
  ],
  [
    "PaymentError::PaymentCollectionNotFound(id) => context.payment_collection_id = Some(*id),",
    "payment collection identity adoption",
  ],
  ["PaymentError::PaymentNotFound(id) => context.payment_id = Some(*id),", "payment identity adoption"],
  ["PaymentError::RefundNotFound(id) => context.refund_id = Some(*id),", "refund identity adoption"],
]) requireText(identityAdoption, value, label);

// --- Orchestration dispatch --------------------------------------------------------------------

const orderChangeVariants = enumVariants(
  orderChangeOrchestration,
  "OrderChangeOrchestrationError",
  "order-change orchestration error enum",
);
if (orderChangeVariants.length !== 3) {
  failures.push(
    `order-change orchestration error enum: expected 3 variants, found ${orderChangeVariants.length}`,
  );
}
requireEveryVariant(
  orderChangeVariants,
  applyMapper,
  "OrderChangeOrchestrationError",
  "order-change apply mapper",
);

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
  "order-change orchestration mapper",
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
  "order-change orchestration mapper",
);

for (const [value, label] of [
  ["OrderChangeOrchestrationError::OrderRead(source)", "order read branch"],
  ["OrderChangeOrchestrationError::OrderCommand(source)", "order command branch"],
  ["OrderChangeOrchestrationError::PostOrder(source)", "post-order branch"],
  [
    '"read_order_change_projection",',
    "order read owner operation",
  ],
  ['"apply_change",', "order command owner operation"],
  ["read_context,", "read context handoff"],
  ["command_context,", "command context handoff"],
  ["context.order_change_id.unwrap_or_default(),", "bounded order-change identity"],
  ["map_admin_order_change_orchestration_error(context, source)", "cross-owner handoff"],
  ["map_admin_order_change_port_error(", "port mapper handoff"],
]) requireTextNormalized(applyMapper, value, `apply mapper: ${label}`);

for (const [value, label] of [
  ["order_error_policy(source)", "order policy handoff"],
  ["payment_error_policy(source)", "payment policy handoff"],
  ["reserved_refund_error_policy(source)", "reserved refund policy handoff"],
  ["admin_order_change_port_error_policy(error)", "owner port policy handoff"],
  ['"rustok_order"', "order source owner"],
  ['"rustok_payment"', "payment source owner"],
  ['"rustok_commerce"', "commerce source owner"],
  ['"commerce_admin_post_order_invalid"', "post-order validation code"],
  ['"Post-order request is invalid"', "post-order validation message"],
  ["context.refund_id = Some(*refund_id);", "reserved refund identity adoption"],
  ["adopt_order_change_order_error_identity(&mut context, source);", "order identity adoption call"],
  [
    "adopt_order_change_payment_error_identity(&mut context, source);",
    "payment identity adoption call",
  ],
]) requireText(orchestrationMapper, value, `orchestration mapper: ${label}`);

for (const [value, label] of [
  ["let tenant_id_non_nil = !context.tenant_id.is_nil();", "tenant shape"],
  ["let actor_id_non_nil = !context.actor_id.is_nil();", "actor shape"],
  ["let order_id_shape = optional_uuid_shape(context.order_id);", "order shape"],
  ["let order_change_id_shape = optional_uuid_shape(context.order_change_id);", "order-change shape"],
  [
    "let payment_collection_id_shape = optional_uuid_shape(context.payment_collection_id);",
    "payment collection shape",
  ],
  ["let payment_id_shape = optional_uuid_shape(context.payment_id);", "payment shape"],
  ["let refund_id_shape = optional_uuid_shape(context.refund_id);", "refund shape"],
  ["owner = ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER,", "orchestration owner"],
  ["source_owner,", "source owner field"],
  ["operation = context.operation,", "operation field"],
  ["error_kind,", "error kind field"],
  ["public_code = code,", "public code field"],
  ["status = %status,", "status field"],
  ["boundary = ADMIN_ORDER_CHANGE_BOUNDARY,", "boundary field"],
  [
    '"commerce admin order change orchestration failed with bounded diagnostics"',
    "mapper message",
  ],
  ["HttpError::new(status, code, message)", "single static envelope constructor"],
]) requireText(orchestrationMapper, value, `orchestration mapper: ${label}`);

for (const forbidden of ["format!(", ".to_string()", "error.message", "?error", "%error"]) {
  forbidText(orchestrationMapper, forbidden, "raw orchestration mapper payload");
}

// --- Route wiring ------------------------------------------------------------------------------

for (const [value, label] of [
  ["OrderChangeOrchestrationService::from_order_ports(", "owner-port orchestration"],
  ["runtime.order_read_port(),", "host-selected Order read port"],
  ["runtime.order_post_order_command_port(),", "host-selected Order command port"],
  [".apply_order_change_with_owner_ports(", "owner-port apply call"],
  ["read_context.clone(),", "read context forwarding"],
  ["command_context.clone(),", "command context forwarding"],
  ["map_admin_order_change_apply_error(", "apply mapper handoff"],
  ["AdminOrderChangeOrchestrationErrorContext::new(", "error context construction"],
  ['"apply_order_change",', "apply operation label"],
  ["require_idempotency_key(&headers)?", "idempotency key"],
  ["[Permission::ORDERS_UPDATE]", "update permission"],
]) requireTextNormalized(applyRoute, value, `apply route: ${label}`);

requireCount(controller, /tracing::error!\(/g, 2, "two owner mapper error lines");
requireCount(controller, /tracing::debug!\(/g, 1, "one bounded entry line");
forbidRawContextLogging(failures, controller, "admin order-change controller");

// --- Shared policies ---------------------------------------------------------------------------

for (const [value, label] of [
  ["pub(crate) fn order_error_policy(", "shared order policy"],
  ["pub(crate) fn payment_error_policy(", "shared payment policy"],
  ["pub(crate) fn reserved_refund_error_policy(", "shared reserved refund policy"],
  ['"commerce_admin_refund_reconciliation_required"', "reserved reconciliation code"],
  ['"Refund remains reserved while the provider outcome is reconciled"',
   "reserved reconciliation message"],
  ['"commerce_admin_refund_provider_unavailable"', "reserved retry code"],
  ['"Refund remains reserved and the provider operation may be retried safely"',
   "reserved retry message"],
]) requireText(adminMod, value, label);

if (failures.length > 0) {
  console.error("Commerce admin order-change owner-port orchestration error verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Admin order-change orchestration keeps owner-typed causes, shared static policies, and bounded diagnostics",
);
