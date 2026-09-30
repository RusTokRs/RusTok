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
const facade = read("crates/modules/rustok-commerce/src/services/checkout_compensation_error_safe.rs");
const owner = read("crates/modules/rustok-commerce/src/services/checkout_compensation_owner_ports.rs");
const sweep = read("crates/modules/rustok-commerce/src/services/checkout_compensation_sweep.rs");
const runtime = read("crates/modules/rustok-commerce/src/controllers/mod.rs");
const evidence = JSON.parse(
  read(
    "crates/modules/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json",
  ),
);
const doc = read("crates/modules/rustok-commerce/docs/admin-checkout-operation-diagnostic-safety.md");
const plan = read("crates/modules/rustok-commerce/docs/implementation-plan.md");

const logger = between(
  controller,
  "fn admin_checkout_operation_http_error(",
  "fn map_operation_error(",
  "checkout logger",
);

for (const value of [
  "let context = AdminCheckoutOperationDiagnosticContext::from(context);",
  "let (status, code, message, error_kind) = policy;",
  "owner = ADMIN_CHECKOUT_OPERATION_OWNER",
  "source_owner,",
  "tenant_state = context.tenant_state",
  "actor_state = context.actor_state",
  "checkout_operation_state = context.checkout_operation_state",
  "reservation_state = context.reservation_state",
  "payment_collection_state = context.payment_collection_state",
  "payment_state = context.payment_state",
  "refund_state = context.refund_state",
  "order_state = context.order_state",
  "order_return_state = context.order_return_state",
  "order_change_state = context.order_change_state",
  "operation = context.operation",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY",
  "HttpError::new(status, code, message)",
]) need(logger, value, "bounded checkout logger");

for (const value of [
  "error = ?error",
  'let error = "redacted"',
  "error.message",
  "error.to_string()",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
  "payment_collection_id = ?context.payment_collection_id",
  "order_id = ?context.order_id",
]) forbid(logger, value, "raw checkout logger payload");

for (const value of [
  "fn require_idempotency_key(headers: &HeaderMap)",
  '.get("Idempotency-Key")',
  "checkout_operation_idempotency_key_required",
  "checkout_operation_idempotency_key_invalid",
  "value.len() > ADMIN_CHECKOUT_OPERATION_MAX_IDEMPOTENCY_KEY_LENGTH",
]) need(controller, value, "caller-owned checkout idempotency boundary");

for (const value of [
  "runtime.checkout_inventory_reservation_port()",
  "runtime.cart_checkout_port()",
  "runtime.checkout_payment_compensation_port()",
  "runtime.checkout_order_compensation_port()",
]) need(controller, value, "typed checkout owner ports in Admin controller");

for (const value of [
  "rustok_inventory::in_process_inventory_reservation_identity_port(",
  "in_process_cart_checkout_port(",
  ".with_payment_provider_registry(",
  "Uuid::new_v4()",
]) forbid(controller, value, "legacy/direct Admin checkout compensation construction");

for (const value of [
  "pub fn new(",
  "payment_compensation_port: Arc<dyn CanonicalCheckoutPaymentCompensationPort>",
  "order_compensation_port: Arc<dyn CanonicalCheckoutOrderCompensationPort>",
  "rustok_payment_shim::wrap_checkout_payment_compensation_port(",
  "rustok_order_shim::wrap_checkout_order_compensation_port(",
  "idempotency_key: impl Into<String>",
]) need(facade, value, "safe compensation facade owner boundary");

for (const value of [
  "OrderService",
  "PaymentService",
  "PaymentProviderOperationJournal",
  "PaymentOrchestrationService",
  "CheckoutOrderIdentityPort",
  "in_process_checkout_order_identity_port",
]) forbid(facade, value, "foreign owner implementation in safe facade");

for (const value of [
  "payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>",
  "order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>",
  "idempotency_key: impl Into<String>",
  "payment_context(tenant_id, actor_id, operation, self.port_deadline, idempotency_key)",
  "order_context(tenant_id, actor_id, operation, self.port_deadline, idempotency_key)",
  ".with_idempotency_key(idempotency_key.to_string())",
]) need(owner, value, "typed compensation owner-port propagation");

for (const value of [
  "order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,",
  "payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>,",
  "order_compensation_port: in_process_checkout_order_compensation_port(",
  "payment_compensation_port: in_process_checkout_payment_compensation_port(",
]) {
  forbid(
    between(
      owner,
      "impl CheckoutCompensationService {",
      "pub async fn compensate(",
      "owner compensation constructor/setters",
    ),
    value,
    "foreign owner construction in active compensation constructor",
  );
}

for (const value of [
  "request_idempotency_key: impl AsRef<str>",
  "fn per_operation_idempotency_key(",
  "operation_key = per_operation_idempotency_key(request_key, operation.id)",
  "CheckoutCompensationService::new(",
  "idempotency_key,",
]) need(sweep, value, "bounded compensation sweep identity");

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

for (const [key, expected] of Object.entries({
  error_redacted_marker_logged: false,
  typed_owner_compensation_ports: true,
  caller_owned_idempotency_required: true,
  sweep_idempotency_bounded: true,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push("evidence.source_contract." + key + " must be " + expected);
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
    failures.push("evidence.validation." + key + " must remain false");
  }
}

for (const value of [
  "Status: **source-ready / unvalidated**",
  "The logger no longer serializes the typed error at all.",
  "The active Admin compensation path now uses host-composed typed owner ports",
  "This remains **source-ready / unvalidated**",
]) need(doc, value, "checkout diagnostic documentation");

need(
  plan,
  "## Audit 2026-09-30: active Admin checkout compensation boundary",
  "Commerce plan active checkout compensation audit section",
);
need(
  plan,
  "Run Cargo/Node/format/CI verification",
  "Commerce plan execution evidence remains open",
);

if (failures.length) {
  console.error("Commerce admin checkout-operation diagnostic verification failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log("Commerce admin checkout-operation boundary is source-consistent and execution validation remains open");
