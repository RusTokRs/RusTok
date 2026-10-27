import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  CommerceCheckoutCompletionCutoverError,
  verifyCommerceCheckoutCompletionCutover,
} from "./verify-commerce-checkout-completion-cutover.mjs";

const stageFile =
  "crates/modules/rustok-commerce/src/services/checkout_order_stages.rs";
const ownerFailureFile =
  "crates/modules/rustok-commerce/src/services/checkout_order_owner_failure.rs";
const pipelineFile =
  "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs";
const creationFile =
  "crates/modules/rustok-commerce/src/services/checkout_order_creation.rs";
const confirmationFile =
  "crates/modules/rustok-commerce/src/services/checkout_order_confirmation.rs";

const files = {
  [stageFile]: `
pub struct CheckoutOrderStageExecutor
order_recovery: CheckoutOrderRecoveryAdapter
pub async fn advance_to_payment_ready(
.reserve_and_checkpoint(
.create_pending_and_adopt(
.confirm_and_checkpoint(
pub async fn load_payment_ready_state(
.read_checkout_order(
ReadCheckoutOrderProjectionRequest {
fn order_read_context(
PortContext::new(
PortActor::system()
.with_causation_id(operation_id.to_string())
.with_deadline(Duration::from_secs(CHECKOUT_ORDER_READ_DEADLINE_SECONDS))
fn owner_read_failure(
CheckoutOrderBoundaryFailure::from_port_error(error)
failure.observe_port_error(context, owner_operation, stage)
pub fn bounded_failure(&self) -> Option<(&str, bool)>
pub fn plan_journal(&self)
`,
  [ownerFailureFile]: `
pub(crate) const CHECKOUT_ORDER_OWNER_BOUNDARY
pub(crate) struct CheckoutOrderDiagnosticError
formatter.write_str("redacted")
pub(crate) fn from_owner_error(error: &OrderError) -> Self
pub(crate) fn from_port_error(error: PortError) -> Self
pub(crate) fn observe_owner_error(
pub(crate) fn observe_port_error(
owner_message_shape
owner_message_len
owner_kind = self.kind.label()
owner_code = %self.code
owner_retryable = self.retryable
error = ?diagnostic
"Checkout order request is invalid"
"Checkout order resource was not found"
"Checkout order state conflicts with the requested operation"
"Checkout order operation is not permitted"
"Checkout order service is temporarily unavailable"
"Checkout order operation could not be completed safely"
`,
  [pipelineFile]: `
self.order_stage
            .load_payment_ready_state
`,
  "crates/modules/rustok-commerce/src/services/checkout_inventory_order_adoption.rs": `
order.status_kind(),
OrderStatusKind::Pending | OrderStatusKind::Confirmed
adopt_and_checkpoint(
expected_stage: CheckoutOperationStage::InventoryReserved
`,
  [creationFile]: `
OrderBoundary {
fn owner_failure(
CheckoutOrderBoundaryFailure::from_owner_error(&error)
failure.observe_owner_error(
pub fn bounded_failure(&self) -> Option<(&str, bool)>
`,
  [confirmationFile]: `
OrderBoundary {
fn owner_failure(
CheckoutOrderBoundaryFailure::from_owner_error(&error)
failure.observe_owner_error(
pub fn bounded_failure(&self) -> Option<(&str, bool)>
`,
  "crates/modules/rustok-order/src/checkout_order_recovery.rs": `
pub struct CheckoutOrderRecoveryAdapter
CheckoutOrderIdentityPort
read_by_operation(
recover_existing_checkout(
read_checkout_order(
owner_hashes_match
legacy_hashes_match
PortError::unavailable(
PortError::conflict(
`,
  "crates/modules/rustok-order/src/ports.rs": `
trait CheckoutCompletionPort
struct InProcessCheckoutCompletionPort
read_checkout_result_by_operation(
`,
  "crates/modules/rustok-order/src/lib.rs": `
pub mod checkout_order_recovery;
pub use checkout_order_recovery::{
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

test("accepts the bounded order-stage cutover", () => {
  const root = fixture();
  assert.equal(verifyCommerceCheckoutCompletionCutover({ root }).status, "ok");
});

test("rejects a restored legacy order-stage source", () => {
  const root = fixture({
    "crates/modules/rustok-commerce/src/services/checkout_order_stages_legacy.rs":
      "include!();\n",
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("retired source must not return"),
  );
});

test("rejects a restored dead pipeline duplicate", () => {
  const root = fixture({
    "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline.rs": "\n",
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes(
        "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline.rs: retired source must not return",
      ),
  );
});

test("rejects a stage that owns the order service", () => {
  const root = fixture({
    [stageFile]: `${files[stageFile]}\norder_service: OrderService,\n`,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden OrderService"),
  );
});

test("rejects a raw owner error interpolation in the stage", () => {
  const root = fixture({
    [stageFile]: `${files[stageFile]}\n        format!("failed: {err}")\n`,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden : {err}"),
  );
});

test("rejects a raw owner error field in the creation executor", () => {
  const root = fixture({
    [creationFile]: `${files[creationFile]}\n    Order(#[from] OrderError),\n`,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden Order(#[from] OrderError)"),
  );
});

test("rejects a pipeline-owned OrderService", () => {
  const root = fixture({
    [pipelineFile]: `${files[pipelineFile]}\nOrderService\n`,
  });
  assert.throws(
    () => verifyCommerceCheckoutCompletionCutover({ root }),
    (error) =>
      error instanceof CommerceCheckoutCompletionCutoverError &&
      error.message.includes("forbidden OrderService"),
  );
});
