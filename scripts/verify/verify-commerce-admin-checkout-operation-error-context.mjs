#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root =
  process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const read = (p) => readFileSync(path.join(root, p), "utf8");
const failures = [];

const need = (s, v, l) => {
  if (!s.includes(v)) failures.push(l + ": missing " + v);
};
const forbid = (s, v, l) => {
  if (s.includes(v)) failures.push(l + ": forbidden " + v);
};
const between = (s, a, b, l) => {
  const i = s.indexOf(a);
  const j = s.indexOf(b, i + a.length);
  if (i < 0 || j < 0) {
    failures.push(l + ": unable to isolate block");
    return "";
  }
  return s.slice(i, j);
};

const controller = read("crates/modules/rustok-commerce/src/controllers/admin/checkout_operations.rs");
const service = read("crates/modules/rustok-commerce/src/services/checkout_compensation.rs");
const sweep = read("crates/modules/rustok-commerce/src/services/checkout_compensation_sweep.rs");
const runtime = read("crates/modules/rustok-commerce/src/controllers/mod.rs");

const show = between(
  controller,
  "pub async fn show_checkout_operation(",
  "pub async fn compensate_checkout_operation(",
  "show route",
);
const compensate = between(
  controller,
  "pub async fn compensate_checkout_operation(",
  "pub async fn sweep_checkout_compensations(",
  "compensate route",
);
const sweepRoute = between(
  controller,
  "pub async fn sweep_checkout_compensations(",
  "fn map_operation(",
  "sweep route",
);

need(show, "[Permission::ORDERS_READ]", "show permission");
need(show, ".get(tenant.id, id)", "show journal read");
need(show, "map_operation_error(", "show mapper");

for (const value of [
  "headers: HeaderMap,",
  "require_idempotency_key(&headers)?",
  "runtime.checkout_inventory_reservation_port()",
  "runtime.cart_checkout_port()",
  "runtime.checkout_payment_compensation_port()",
  "runtime.checkout_order_compensation_port()",
  "CheckoutCompensationService::new(",
  ".compensate(",
  "idempotency_key.clone()",
  "idempotency_key,",
]) need(compensate, value, "compensate route contract");

for (const value of [
  "headers: HeaderMap,",
  "require_idempotency_key(&headers)?",
  "runtime.checkout_inventory_reservation_port()",
  "runtime.cart_checkout_port()",
  "runtime.checkout_payment_compensation_port()",
  "runtime.checkout_order_compensation_port()",
  "CheckoutCompensationSweepService::new(",
  ".run(",
  "idempotency_key,",
  "input.limit",
]) need(sweepRoute, value, "sweep route contract");

for (const value of [
  "rustok_inventory::in_process_inventory_reservation_identity_port(",
  "in_process_cart_checkout_port(",
  ".with_payment_provider_registry(",
  "Uuid::new_v4()",
]) forbid(controller, value, "direct/generated checkout compensation dependency");

for (const value of [
  "PaymentService",
  "OrderService",
  "PaymentProviderOperationJournal",
  "CheckoutOrderIdentityPort",
  "in_process_checkout_order_identity_port",
]) forbid(service, value, "foreign owner service in Commerce compensation");

for (const value of [
  "payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>",
  "order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>",
  "pub async fn compensate(",
  "idempotency_key: impl Into<String>",
  "self.payment_compensation_port",
  ".compensate_checkout_payment(",
  "self.order_compensation_port",
  ".compensate_checkout_order(",
  "with_idempotency_key(idempotency_key.to_string())",
  "PortActor::user(actor_id.to_string())",
]) need(service, value, "typed compensation orchestration");

for (const value of [
  "pub async fn run(",
  "request_idempotency_key: impl AsRef<str>",
  "operation_key = format!(",
  ".compensate(",
  "operation_key,",
]) need(sweep, value, "replay-safe compensation sweep");

for (const value of [
  "checkout_payment_compensation_port:",
  "checkout_order_compensation_port:",
  "checkout_inventory_reservation_port:",
  "cart_checkout_port:",
  "shared_get::<std::sync::Arc<dyn rustok_payment::CheckoutPaymentCompensationPort>>()",
  "shared_get::<std::sync::Arc<dyn rustok_order::CheckoutOrderCompensationPort>>()",
  "shared_get::<std::sync::Arc<dyn rustok_inventory::InventoryReservationIdentityPort>>()",
  "shared_get::<std::sync::Arc<dyn rustok_cart::CartCheckoutPort>>()",
]) need(runtime, value, "Commerce host checkout compensation composition");

for (const value of [
  "CheckoutCompensationError::Operation(source)",
  "CheckoutCompensationError::ReservationJournal(source)",
  "CheckoutCompensationError::ManualReconciliation(_)",
  "CheckoutCompensationError::Conflict(_)",
  "CheckoutCompensationError::Boundary {",
  "CheckoutCompensationError::CompensationAndJournal { .. }",
  "checkout_reconciliation_required",
  "checkout_compensation_conflict",
  "checkout_compensation_pending",
  "HttpError::new(status, code, message)",
]) need(controller, value, "preserved checkout compensation error policy");

for (const value of [
  "error = ?error",
  "error.message",
  "error.to_string()",
  "format!(",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
]) forbid(controller, value, "raw checkout public/error conversion");

if (failures.length) {
  console.error("Commerce admin checkout-operation error-context verification failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log("Commerce admin checkout operations use typed compensation ports and caller-owned replay identity");
