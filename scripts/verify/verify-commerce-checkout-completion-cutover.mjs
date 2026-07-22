import fs from "node:fs";
import path from "node:path";
import process from "node:process";

export class CommerceCheckoutCompletionCutoverError extends Error {
  constructor(message) {
    super(message);
    this.name = "CommerceCheckoutCompletionCutoverError";
  }
}

const defaultRoot = process.cwd();

const files = {
  stage: "crates/rustok-commerce/src/services/checkout_order_stages.rs",
  pipeline: "crates/rustok-commerce/src/services/checkout_stage_pipeline.rs",
  adoption: "crates/rustok-commerce/src/services/checkout_inventory_order_adoption.rs",
  recovery: "crates/rustok-order/src/checkout_order_recovery.rs",
  ports: "crates/rustok-order/src/ports.rs",
  orderLib: "crates/rustok-order/src/lib.rs",
};

const requireMarker = (failures, source, marker, file) => {
  if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
};

const forbidMarker = (failures, source, marker, file) => {
  if (source.includes(marker)) failures.push(`${file}: forbidden ${marker}`);
};

export function verifyCommerceCheckoutCompletionCutover({ root = defaultRoot } = {}) {
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  const sources = Object.fromEntries(
    Object.entries(files).map(([key, file]) => [key, read(file)]),
  );
  const failures = [];

  for (const marker of [
    "CheckoutCompletionPort",
    "complete_checkout(write_context, request)",
    "CheckoutOrderRecoveryAdapter",
    "recover_existing_checkout(",
    "read_checkout_order(",
    "CompleteCheckoutPortRequest",
    ".with_causation_id(operation_id.to_string())",
    ".with_idempotency_key(",
    ".with_deadline(deadline)",
    "expected_stage: CheckoutOperationStage::InventoryReserved",
    "next_stage: CheckoutOperationStage::OrderCreated",
    "expected_stage: CheckoutOperationStage::OrderCreated",
    "next_stage: CheckoutOperationStage::PaymentReady",
  ]) {
    requireMarker(failures, sources.stage, marker, files.stage);
  }
  for (const marker of [
    "CheckoutOrderCreationExecutor",
    "CheckoutOrderConfirmationExecutor",
    "OrderService::new",
    "order_service",
  ]) {
    forbidMarker(failures, sources.stage, marker, files.stage);
  }

  requireMarker(
    failures,
    sources.pipeline,
    ".order_stage\n            .load_payment_ready_state",
    files.pipeline,
  );
  forbidMarker(failures, sources.pipeline, "OrderService", files.pipeline);
  forbidMarker(failures, sources.pipeline, "order_service", files.pipeline);

  for (const marker of [
    'matches!(order.status.as_str(), "pending" | "confirmed")',
    "adopt_and_checkpoint(",
    "expected_stage: CheckoutOperationStage::InventoryReserved",
  ]) {
    requireMarker(failures, sources.adoption, marker, files.adoption);
  }

  for (const marker of [
    "pub struct CheckoutOrderRecoveryAdapter",
    "CheckoutOrderIdentityPort",
    "read_by_operation(",
    "adopt_legacy(",
    "recover_existing_checkout(",
    "read_checkout_order(",
    "owner_hashes_match",
    "legacy_hashes_match",
    "PortError::unavailable(",
    "PortError::conflict(",
  ]) {
    requireMarker(failures, sources.recovery, marker, files.recovery);
  }
  requireMarker(
    failures,
    sources.orderLib,
    "pub mod checkout_order_recovery;",
    files.orderLib,
  );
  requireMarker(
    failures,
    sources.orderLib,
    "pub use checkout_order_recovery::*;",
    files.orderLib,
  );

  for (const marker of [
    "trait CheckoutCompletionPort",
    "struct InProcessCheckoutCompletionPort",
    "read_checkout_result_by_operation(",
  ]) {
    requireMarker(failures, sources.ports, marker, files.ports);
  }

  if (failures.length > 0) {
    throw new CommerceCheckoutCompletionCutoverError(failures.join("\n"));
  }

  return {
    status: "ok",
    checked_files: Object.values(files),
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    console.log(JSON.stringify(verifyCommerceCheckoutCompletionCutover(), null, 2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
