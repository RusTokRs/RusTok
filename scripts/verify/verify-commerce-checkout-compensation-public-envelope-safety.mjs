#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const paths = {
  source: "crates/rustok-commerce/src/services/checkout_compensation.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/checkout-compensation-public-envelope-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/checkout-compensation-public-envelope-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return "";
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  '#[error("checkout compensation requires manual reconciliation: {0}")]',
  "ManualReconciliation(String)",
  '#[error("checkout compensation conflict: {0}")]',
  "Conflict(String)",
  "Payment(#[from] PaymentError)",
  "PaymentOrchestration(#[from] PaymentOrchestrationError)",
  "Order(#[from] OrderError)",
  "Boundary {",
  "CompensationAndJournal {",
]) requireText(source, marker, `${paths.source}: preserved error shape`);

const manualHelper = functionBody(source, "manual_reconciliation");
for (const marker of [
  "message: &'static str",
  "CheckoutCompensationError::ManualReconciliation(message.to_string())",
]) requireText(manualHelper, marker, `${paths.source}: manual helper`);

const conflictHelper = functionBody(source, "compensation_conflict");
for (const marker of [
  "message: &'static str",
  "CheckoutCompensationError::Conflict(message.to_string())",
]) requireText(conflictHelper, marker, `${paths.source}: conflict helper`);

requireCount(source, "manual_reconciliation(", 8, "seven manual sites plus helper");
requireCount(source, "compensation_conflict(", 11, "ten conflict sites plus helper");

for (const message of [
  "checkout operation cannot be claimed for compensation",
  "captured checkout state requires refund reconciliation",
  "checkout order identity is missing",
  "payment provider operation requires reconciliation",
  "captured payment collection requires reconciliation",
  "payment collection state does not allow compensation",
  "order checkpoint does not match checkout operation",
  "order state requires manual cancellation reconciliation",
  "order state does not allow compensation",
  "inventory release result does not match reservation",
  "consumed inventory reservation requires reconciliation",
  "inventory reservation state does not allow compensation",
  "cart release did not restore the active state",
  "completed cart requires reconciliation",
  "cart state does not allow release",
  "order identity does not match checkout operation",
  "checkout stage does not allow compensation",
]) requireText(source, `"${message}"`, `${paths.source}: stable branch reason`);

for (const forbidden of [
  "CheckoutCompensationError::ManualReconciliation(format!",
  "CheckoutCompensationError::Conflict(format!",
  "cannot be claimed for compensation; status=",
  "captured funds must be reconciled through refund policy",
  "but has no order-owner identity",
  "payment provider operation {} is",
  "payment collection {collection_id}",
  "order {order_id}",
  "checkout reservation {}",
  "inventory reservation {}",
  "cart {}",
  "typed order identity does not match checkout operation {}",
  "unsupported checkout stage `{other}`",
  "lease_owner={}",
]) forbidText(source, forbidden, `${paths.source}: dynamic state envelope`);

const compensate = functionBody(source, "compensate");
for (const marker of [
  ".claim_compensation(",
  "if current.status == CheckoutOperationStatus::Compensated.as_str()",
  "return Ok(current);",
  "compensation_error_code(&compensation)",
  "let message = compensation.to_string();",
  ".mark_compensation_retryable(",
  "Ok(_) => Err(compensation)",
  "CheckoutCompensationError::CompensationAndJournal",
]) requireText(compensate, marker, `${paths.source}: preserved compensation journal flow`);
requireOrder(
  compensate,
  [
    "compensation_error_code(&compensation)",
    "let message = compensation.to_string();",
    ".mark_compensation_retryable(",
  ],
  `${paths.source}: journal code-message-mutation order`,
);

const payment = functionBody(source, "compensate_payment");
for (const marker of [
  ".list_by_collection(tenant_id, collection_id)",
  "provider_operations.iter().any(|provider_operation|",
  "PROVIDER_OPERATION_EXECUTING",
  "PROVIDER_OPERATION_SUCCEEDED",
  "PROVIDER_OPERATION_RECONCILIATION_REQUIRED",
  ".cancel_collection(",
  'reason: Some("checkout_compensation".to_string())',
]) requireText(payment, marker, `${paths.source}: preserved payment behavior`);

const order = functionBody(source, "compensate_order");
for (const marker of [
  ".get_order_with_locale_fallback(",
  "operation.order_id.is_some() && operation.order_id != Some(order.id)",
  '"pending" | "confirmed"',
  ".cancel_order(",
  'Some("checkout_compensation".to_string())',
  '"paid" | "shipped" | "delivered"',
]) requireText(order, marker, `${paths.source}: preserved order behavior`);

const inventory = functionBody(source, "release_remaining_reservations");
for (const marker of [
  ".list_by_operation(tenant_id, operation.id)",
  "CheckoutInventoryReservationStatus::Reserved.as_str()",
  ".release_inventory_by_identity(",
  "released.reservation_id != reservation.reservation_id",
  "released.external_id != reservation.external_id",
  "released.variant_id != reservation.variant_id",
  ".mark_released(tenant_id, reservation.reservation_id)",
  "CheckoutInventoryReservationStatus::Consumed.as_str()",
]) requireText(inventory, marker, `${paths.source}: preserved inventory behavior`);

const cart = functionBody(source, "release_cart");
for (const marker of [
  ".read_cart_checkout_snapshot(",
  "CartStatus::CheckingOut.as_str()",
  ".release_cart_checkout(",
  "released.status != CartStatus::Active.as_str()",
  "CartStatus::Active.as_str()",
  "CartStatus::Completed.as_str()",
]) requireText(cart, marker, `${paths.source}: preserved cart behavior`);

const boundary = functionBody(source, "boundary_error");
for (const marker of [
  "CheckoutCompensationError::Boundary {",
  "stage,",
  "code: error.code",
  "message: error.message",
  "retryable: error.retryable",
]) requireText(boundary, marker, `${paths.source}: unchanged boundary envelope`);

const codes = functionBody(source, "compensation_error_code");
for (const marker of [
  '"checkout.compensation_manual_reconciliation"',
  '"checkout.compensation_boundary_failed"',
  '"checkout.compensation_payment_failed"',
  '"checkout.compensation_order_failed"',
  '"checkout.compensation_inventory_failed"',
  '"checkout.compensation_failed"',
]) requireText(codes, marker, `${paths.source}: preserved compensation code`);

if (
  evidence.status !==
  "commerce_checkout_compensation_public_envelope_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  manual_reconciliation_state_site_count: 7,
  conflict_state_site_count: 10,
  total_state_envelope_site_count: 17,
  dynamic_uuid_in_state_envelopes: false,
  dynamic_status_in_state_envelopes: false,
  dynamic_lease_owner_in_state_envelopes: false,
  stable_branch_specific_messages: true,
  compensation_error_codes_preserved: true,
  journal_message_persistence_preserved: true,
  transparent_owner_error_envelopes_closed: false,
  boundary_error_envelope_closed: false,
  compensation_and_journal_envelope_closed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
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
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "seven manual-reconciliation branches and ten conflict branches", `${paths.doc}: site count`);
requireText(doc, "They also no longer include runtime status or lease-owner values.", `${paths.doc}: payload policy`);
requireText(doc, "leaves `CheckoutCompensationError::Boundary`", `${paths.doc}: remaining boundary`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce checkout compensation public-envelope verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce checkout compensation state-derived manual-reconciliation and conflict envelopes are stable and payload-free; execution validation remains open",
);
