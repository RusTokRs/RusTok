#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() || path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (p) => readFileSync(path.join(root, p), "utf8");
const failures = [];
const need = (s, v, l) => { if (!s.includes(v)) failures.push(l + ": missing " + v); };
const forbid = (s, v, l) => { if (s.includes(v)) failures.push(l + ": forbidden " + v); };
const between = (s, a, b, l) => { const i=s.indexOf(a); const j=s.indexOf(b, i+a.length); if(i<0||j<0){ failures.push(l + ": unable to isolate block"); return ""; } return s.slice(i,j); };

const controller = read("crates/modules/rustok-commerce/src/controllers/admin/checkout_operations.rs");
const service = read("crates/modules/rustok-commerce/src/services/checkout_compensation_error_safe.rs");
const sweep = read("crates/modules/rustok-commerce/src/services/checkout_compensation_sweep.rs");
const evidence = JSON.parse(read("crates/modules/rustok-commerce/contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json"));
const doc = read("crates/modules/rustok-commerce/docs/admin-checkout-operation-diagnostic-safety.md");
const plan = read("crates/modules/rustok-commerce/docs/implementation-plan.md");

const logger = between(controller, "fn admin_checkout_operation_http_error(", "fn map_operation_error(", "checkout logger");
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
  "let error = \"redacted\"",
  "error.message",
  "error.to_string()",
  "format!(",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "checkout_operation_id = ?context.checkout_operation_id",
  "payment_collection_id = ?context.payment_collection_id",
  "order_id = ?context.order_id",
]) forbid(logger, value, "raw checkout logger payload");

for (const value of [
  "fn require_idempotency_key(headers: &HeaderMap)",
  "headers.get(\"Idempotency-Key\")",
  "checkout_operation_idempotency_key_required",
  "checkout_operation_idempotency_key_invalid",
  "value.len() > ADMIN_CHECKOUT_OPERATION_MAX_IDEMPOTENCY_KEY_LENGTH",
  "runtime.checkout_inventory_reservation_port()",
  "runtime.cart_checkout_port()",
  "runtime.checkout_payment_compensation_port()",
  "runtime.checkout_order_compensation_port()",
]) need(controller, value, "Admin checkout compensation boundary");

for (const value of [
  "rustok_inventory::in_process_inventory_reservation_identity_port(",
  "in_process_cart_checkout_port(",
  ".with_payment_provider_registry(",
  "Uuid::new_v4()",
]) forbid(controller, value, "legacy/direct Admin checkout compensation construction");

for (const value of [
  "inner: legacy::CheckoutCompensationService",
  "CheckoutPaymentCompensationPort",
  "CheckoutOrderCompensationPort",
  "rustok_order_shim::wrap_checkout_order_compensation_port",
  "with_idempotency_key(idempotency_key.to_string())",
  "PortActor::user(actor_id.to_string())",
]) need(service, value, "Commerce compensation owner-port boundary");
for (const value of [
  "legacy::CheckoutCompensationService::new",
  "CheckoutCompensationService::new",
  "CheckoutCompensationService::new",
  "CheckoutCompensationService::new",
  "CheckoutCompensationService::new",
  "PaymentOrchestrationService",
]) forbid(service, value, "foreign service retained in Commerce compensation service");

for (const value of [
  "request_idempotency_key: impl AsRef<str>",
  "operation_key = format!(",
  "CheckoutCompensationService::new(",
  "idempotency_key",
]) need(sweep, value, "Checkout compensation sweep replay boundary");

if (evidence.source_contract?.error_redacted_marker_logged !== false) failures.push("evidence.source_contract.error_redacted_marker_logged must be false");
for (const key of ["tests_run","cargo_run","format_run","verifiers_run","workflow_checks_run","ci_run","compile_proven","runtime_proven"]) {
  if (evidence.validation?.[key] !== false) failures.push("evidence.validation." + key + " must remain false");
}
for (const value of [
  "Status: **source-ready / unvalidated**",
  "Optional checkout, reservation, payment, refund, order, return, and change UUIDs are represented only as `absent`, `present_nil`, or `present_non_nil`.",
  "The broader ecommerce correlation-safe mapper task remains open.",
]) need(doc, value, "checkout diagnostic documentation");
need(plan, "Continue the broader correlation-safe mapper cleanup", "Commerce plan broader mapper cleanup");

if (failures.length) {
  console.error("Commerce admin checkout-operation diagnostic verification failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(Math.min(failures.length, 255));
}
console.log("Commerce admin checkout-operation diagnostics are bounded; execution validation remains open");