import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  CommerceCheckoutCompletionCutoverError,
  verifyCommerceCheckoutCompletionCutover,
} from "./verify-commerce-checkout-completion-cutover.mjs";

const files = {
  "crates/rustok-commerce/src/services/checkout_order_stages.rs": `
CheckoutCompletionPort
complete_checkout(write_context, request)
CheckoutOrderRecoveryAdapter
recover_existing_checkout(
read_checkout_order(
CompleteCheckoutPortRequest
.with_causation_id(operation_id.to_string())
.with_idempotency_key(
.with_deadline(deadline)
expected_stage: CheckoutOperationStage::InventoryReserved
next_stage: CheckoutOperationStage::OrderCreated
expected_stage: CheckoutOperationStage::OrderCreated
next_stage: CheckoutOperationStage::PaymentReady
`,
  "crates/rustok-commerce/src/services/checkout_stage_pipeline.rs": `
self.order_stage
            .load_payment_ready_state
`,
  "crates/rustok-commerce/src/services/checkout_inventory_order_adoption.rs": `
matches!(order.status.as_str(), "pending" | "confirmed")
adopt_and_checkpoint(
expected_stage: CheckoutOperationStage::InventoryReserved
`,
  "crates/rustok-order/src/checkout_order_recovery.rs": `
pub struct CheckoutOrderRecoveryAdapter
CheckoutOrderIdentityPort
read_by_operation(
adopt_legacy(
recover_existing_checkout(
read_checkout_order(
owner_hashes_match
legacy_hashes_match
PortError::unavailable(
PortError::conflict(
`,
  "crates/rustok-order/src/ports.rs": `
trait CheckoutCompletionPort
struct InProcessCheckoutCompletionPort
read_checkout_result_by_operation(
`,
  "crates/rustok-order/src/lib.rs": `
pub mod checkout_order_recovery;
pub use checkout_order_recovery::*;
`,
};

function fixture(overrides = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "commerce-order-cutover-"));
  for (const [relative, content] of Object.entries({ ...files, ...overrides })) {
    const target = path.join(root, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
  return root;
}

test("accepts staged checkout completion cutover", () => {
  const root = fixture();
  assert.equal(verifyCommerceCheckoutCompletionCutover({ root }).status, "ok");
});

test("rejects a restored direct order creation executor", () => {
  const stage = `${files["crates/rustok-commerce/src/services/checkout_order_stages.rs"]}\nCheckoutOrderCreationExecutor`;
  const root = fixture({
    "crates/rustok-commerce/src/services/checkout_order_stages.rs": stage,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden CheckoutOrderCreationExecutor"),
  );
});

test("rejects a pipeline-owned OrderService", () => {
  const pipeline = `${files["crates/rustok-commerce/src/services/checkout_stage_pipeline.rs"]}\nOrderService`;
  const root = fixture({
    "crates/rustok-commerce/src/services/checkout_stage_pipeline.rs": pipeline,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden OrderService"),
  );
});
