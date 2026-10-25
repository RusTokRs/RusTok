#!/usr/bin/env node
/**
 * Commerce checkout owner-stage boundary.
 *
 * Every checkout stage reaches its owner through a port, and every port
 * rejection is projected onto a sanitized stage boundary before the stage mapper
 * sees it:
 *
 * - order: `CheckoutOrderRecoveryAdapter` (owner policy + bounded `PortError`),
 *   projected by `CheckoutOrderStageError::owner_read_failure`;
 * - payment and fulfillment: sanitizing owner adapters mounted by the stage
 *   facades;
 * - order settlement: the sanitizing settlement port owned by Commerce.
 *
 * A stage that owns an owner service directly bypasses the policy and the
 * sanitizer, so `OrderService`, `PaymentService`, `FulfillmentService`, and the
 * provider journal are forbidden in every stage and in the mounted pipeline.
 */
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (file) => readFileSync(new URL(file, root), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireRetired = (file) => {
  if (existsSync(new URL(file, root))) {
    failures.push(`${file}: retired source must not return`);
  }
};

const services = read("crates/modules/rustok-commerce/src/services/mod.rs");
const orderStageFacade = read(
  "crates/modules/rustok-commerce/src/services/checkout_order_stages.rs",
);
const paymentStageFacade = read(
  "crates/modules/rustok-commerce/src/services/checkout_payment_stages.rs",
);
const fulfillmentStageFacade = read(
  "crates/modules/rustok-commerce/src/services/checkout_fulfillment_stages.rs",
);
const pipeline = read(
  "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs",
);
const orderCompletionOwner = read("crates/modules/rustok-order/src/ports.rs");
const orderRecoveryOwner = read("crates/modules/rustok-order/src/checkout_order_recovery.rs");
const paymentOwner = read("crates/modules/rustok-payment/src/checkout_execution/types.rs");
const paymentPolicy = read(
  "crates/modules/rustok-payment/src/checkout_execution/validation_errors.rs",
);
const fulfillmentOwner = read("crates/modules/rustok-fulfillment/src/checkout_execution.rs");
const orderSettlementOwner = read(
  "crates/modules/rustok-order/src/checkout_payment_settlement.rs",
);

requireRetired(
  "crates/modules/rustok-commerce/src/services/checkout_order_stages_legacy.rs",
);
requireRetired(
  "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline.rs",
);

requireText(
  services,
  '#[path = "checkout_stage_pipeline_owner_ports.rs"]',
  "commerce services mount",
);
requireText(
  paymentStageFacade,
  '#[path = "checkout_payment_stages_legacy.rs"]',
  "payment stage legacy mount",
);

for (const [source, label, required] of [
  [
    orderStageFacade,
    "order stage",
    [
      "order_recovery: CheckoutOrderRecoveryAdapter",
      "read_checkout_order(",
      "ReadCheckoutOrderProjectionRequest {",
      "fn owner_read_failure(",
      "CheckoutOrderBoundaryFailure::from_port_error(error)",
    ],
  ],
  [
    paymentStageFacade,
    "payment stage",
    [
      "CheckoutPaymentExecutionPort",
      "prepare_checkout_collection(",
      "authorize_checkout_collection(",
      "capture_checkout_collection(",
      "read_checkout_collection(",
    ],
  ],
  [
    fulfillmentStageFacade,
    "fulfillment stage",
    [
      "CheckoutFulfillmentExecutionPort",
      "CheckoutOrderPaymentSettlementPort",
      "ensure_checkout_fulfillments(",
      "read_checkout_fulfillments(",
      "settle_checkout_payment(",
    ],
  ],
  [
    pipeline,
    "mounted pipeline",
    [
      "self.order_stage\n            .load_payment_ready_state",
      "self.payment_stage\n            .load_payment_captured_state",
      "self.fulfillment_stage\n            .load_fulfillment_created_state",
    ],
  ],
]) {
  for (const value of required) requireText(source, value, label);
}

for (const [value, label] of [
  [".with_causation_id(operation_id.to_string())", "order-stage read causation"],
  [".with_deadline(Duration::from_secs(CHECKOUT_ORDER_READ_DEADLINE_SECONDS))", "order-stage read deadline"],
]) {
  requireText(orderStageFacade, value, label);
}

for (const [value, label] of [
  ["SanitizingCheckoutPaymentExecutionPort", "payment-stage owner adapter"],
  ["mod rustok_payment_shim", "payment-stage owner shim"],
]) {
  requireText(paymentStageFacade, value, label);
}

for (const [value, label] of [
  ['include!("checkout_fulfillment_stages_legacy.rs");', "mounted fulfillment-stage include"],
  ["struct SanitizingCheckoutFulfillmentExecutionPort", "fulfillment-stage owner adapter"],
  ["struct SanitizingCheckoutOrderPaymentSettlementPort", "order-settlement owner adapter"],
]) {
  requireText(fulfillmentStageFacade, value, label);
}

for (const [source, label] of [
  [orderStageFacade, "order stage"],
  [paymentStageFacade, "payment stage"],
  [fulfillmentStageFacade, "fulfillment stage"],
  [pipeline, "mounted pipeline"],
]) {
  for (const value of [
    "PaymentService",
    "FulfillmentService",
    "OrderService",
    "PaymentProviderOperationJournal",
    "PaymentOrchestrationService",
    "FROM fulfillments",
    "SELECT id FROM fulfillments",
  ]) {
    forbidText(source, value, label);
  }
}

for (const [source, label, port, operation] of [
  [
    orderCompletionOwner,
    "order completion owner",
    "CheckoutCompletionPort",
    "complete_checkout(",
  ],
  [
    paymentOwner,
    "payment owner",
    "CheckoutPaymentExecutionPort",
    "prepare_checkout_collection(",
  ],
  [
    fulfillmentOwner,
    "fulfillment owner",
    "CheckoutFulfillmentExecutionPort",
    "ensure_checkout_fulfillments(",
  ],
  [
    orderSettlementOwner,
    "order settlement owner",
    "CheckoutOrderPaymentSettlementPort",
    "mark_paid(",
  ],
]) {
  requireText(source, `trait ${port}`, label);
  requireText(source, operation, label);
}

requireText(
  paymentPolicy,
  ".require_policy(PortCallPolicy::read())",
  "payment owner read policy",
);
requireText(
  paymentPolicy,
  ".require_policy(PortCallPolicy::write())",
  "payment owner write policy",
);
requireText(
  fulfillmentOwner,
  "require_policy(PortCallPolicy::",
  "fulfillment owner policy",
);
requireText(
  orderSettlementOwner,
  "require_policy(PortCallPolicy::",
  "order settlement owner policy",
);

for (const [value, label] of [
  ["pub struct CheckoutOrderRecoveryAdapter", "order recovery adapter"],
  ["recover_existing_checkout(", "order recovery operation"],
  ["read_checkout_order(", "order recovery read operation"],
  ["context.require_policy(PortCallPolicy::write())?;", "order recovery write policy"],
  ["context.require_policy(PortCallPolicy::read())?;", "order recovery read policy"],
]) requireText(orderRecoveryOwner, value, label);


if (failures.length > 0) {
  console.error("Checkout owner-stage boundary verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Checkout order, payment, fulfillment, order settlement, and pipeline recovery use sanitized owner boundaries",
);
