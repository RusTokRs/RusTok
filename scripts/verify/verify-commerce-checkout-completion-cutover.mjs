#!/usr/bin/env node
/**
 * Commerce checkout completion cutover.
 *
 * The order stage drives a claimed checkout operation through inventory
 * reservation, order creation with adoption, and order confirmation until
 * `payment_ready`. Two compatibility layers were retired and must not return:
 * the `checkout_order_stages_legacy.rs` include source, and the dead
 * `checkout_stage_pipeline.rs` duplicate of the mounted owner-port pipeline.
 *
 * The stage reads the owner projection through `CheckoutOrderRecoveryAdapter`
 * and projects owner failures onto the bounded stage boundary, so the mounted
 * source never writes an owner payload into a Commerce error or the operation
 * journal.
 */
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
  stage: "crates/modules/rustok-commerce/src/services/checkout_order_stages.rs",
  ownerFailure:
    "crates/modules/rustok-commerce/src/services/checkout_order_owner_failure.rs",
  pipeline:
    "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs",
  adoption:
    "crates/modules/rustok-commerce/src/services/checkout_inventory_order_adoption.rs",
  creation:
    "crates/modules/rustok-commerce/src/services/checkout_order_creation.rs",
  confirmation:
    "crates/modules/rustok-commerce/src/services/checkout_order_confirmation.rs",
  recovery: "crates/modules/rustok-order/src/checkout_order_recovery.rs",
  ports: "crates/modules/rustok-order/src/ports.rs",
  orderLib: "crates/modules/rustok-order/src/lib.rs",
};

const retiredFiles = [
  "crates/modules/rustok-commerce/src/services/checkout_order_stages_legacy.rs",
  "crates/modules/rustok-commerce/src/services/checkout_stage_pipeline.rs",
];

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

  for (const retired of retiredFiles) {
    if (fs.existsSync(path.join(root, retired))) {
      failures.push(`${retired}: retired source must not return`);
    }
  }

  for (const marker of [
    "pub struct CheckoutOrderStageExecutor",
    "order_recovery: CheckoutOrderRecoveryAdapter",
    "pub async fn advance_to_payment_ready(",
    ".reserve_and_checkpoint(",
    ".create_pending_and_adopt(",
    ".confirm_and_checkpoint(",
    "pub async fn load_payment_ready_state(",
    ".read_checkout_order(",
    "ReadCheckoutOrderProjectionRequest {",
    "fn order_read_context(",
    "PortContext::new(",
    "PortActor::system()",
    ".with_causation_id(operation_id.to_string())",
    ".with_deadline(Duration::from_secs(CHECKOUT_ORDER_READ_DEADLINE_SECONDS))",
    "fn owner_read_failure(",
    "CheckoutOrderBoundaryFailure::from_port_error(error)",
    "failure.observe_port_error(context, owner_operation, stage)",
    "pub fn bounded_failure(&self) -> Option<(&str, bool)>",
    "pub fn plan_journal(&self)",
  ]) {
    requireMarker(failures, sources.stage, marker, files.stage);
  }

  for (const marker of [
    "include!(",
    "CheckoutCompletionPort",
    "SanitizingCheckoutCompletionPort",
    "OrderService",
    "order_service",
    ": {err}",
    ": {error}",
  ]) {
    forbidMarker(failures, sources.stage, marker, files.stage);
  }

  for (const marker of [
    "pub(crate) const CHECKOUT_ORDER_OWNER_BOUNDARY",
    "pub(crate) struct CheckoutOrderDiagnosticError",
    'formatter.write_str("redacted")',
    "pub(crate) fn from_owner_error(error: &OrderError) -> Self",
    "pub(crate) fn from_port_error(error: PortError) -> Self",
    "pub(crate) fn observe_owner_error(",
    "pub(crate) fn observe_port_error(",
    "owner_message_shape",
    "owner_message_len",
    "owner_kind = self.kind.label()",
    "owner_code = %self.code",
    "owner_retryable = self.retryable",
    "Checkout order request is invalid",
    "Checkout order resource was not found",
    "Checkout order state conflicts with the requested operation",
    "Checkout order operation is not permitted",
    "Checkout order service is temporarily unavailable",
    "Checkout order operation could not be completed safely",
  ]) {
    requireMarker(failures, sources.ownerFailure, marker, files.ownerFailure);
  }

  for (const marker of ["message = %", "error = ?error", "error = %error"]) {
    forbidMarker(failures, sources.ownerFailure, marker, files.ownerFailure);
  }

  for (const [source, file] of [
    [sources.creation, files.creation],
    [sources.confirmation, files.confirmation],
  ]) {
    for (const marker of [
      "OrderBoundary {",
      "fn owner_failure(",
      "CheckoutOrderBoundaryFailure::from_owner_error(&error)",
      "failure.observe_owner_error(",
      "pub fn bounded_failure(&self) -> Option<(&str, bool)>",
    ]) {
      requireMarker(failures, source, marker, file);
    }
    for (const marker of ["Order(#[from] OrderError)", ": {error}", ": {err}"]) {
      forbidMarker(failures, source, marker, file);
    }
  }

  requireMarker(
    failures,
    sources.pipeline,
    ".order_stage\n            .load_payment_ready_state",
    files.pipeline,
  );
  forbidMarker(failures, sources.pipeline, "OrderService", files.pipeline);

  for (const marker of [
    "order.status_kind(),",
    "OrderStatusKind::Pending | OrderStatusKind::Confirmed",
    "adopt_and_checkpoint(",
    "expected_stage: CheckoutOperationStage::InventoryReserved",
  ]) {
    requireMarker(failures, sources.adoption, marker, files.adoption);
  }

  for (const marker of [
    "pub struct CheckoutOrderRecoveryAdapter",
    "CheckoutOrderIdentityPort",
    "read_by_operation(",
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
    "pub use checkout_order_recovery::{",
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
    retired_files: retiredFiles,
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
