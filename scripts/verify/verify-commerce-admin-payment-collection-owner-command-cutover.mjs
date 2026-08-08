#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const requireText = (source, text, label) => {
  if (!source.includes(text)) {
    throw new Error(`missing ${label}: ${text}`);
  }
};
const forbidText = (source, text, label) => {
  if (source.includes(text)) {
    throw new Error(`forbidden ${label}: ${text}`);
  }
};

const mounted = read("crates/rustok-commerce/src/controllers/admin/payments_owner_reads.rs");
const legacy = read("crates/rustok-commerce/src/controllers/admin/payments.rs");
const httpRuntime = read("crates/rustok-commerce/src/controllers/mod.rs");
const paymentLib = read("crates/rustok-payment/src/lib.rs");
const ownerCommand = read("crates/rustok-payment/src/admin_collection_command.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");
const doc = read("crates/rustok-commerce/docs/admin-payment-collection-owner-command-cutover-2026-08-09.md");

requireText(paymentLib, "mod admin_collection_command;", "Payment admin collection command module");
requireText(paymentLib, "PaymentAdminCollectionCommandPort, PaymentAdminCollectionCommandRuntime", "Payment command exports");
requireText(ownerCommand, "pub trait PaymentAdminCollectionCommandPort", "Payment owner command port");
requireText(ownerCommand, "pub struct PaymentAdminCollectionCommandRuntime", "Payment owner command runtime");
requireText(ownerCommand, "context.require_policy(PortCallPolicy::write())", "write-port admission");
requireText(ownerCommand, "PaymentService::new(db.clone())", "owner-local Payment service construction");
requireText(ownerCommand, "PaymentProviderOperationJournal::new(db)", "owner-local provider journal construction");
requireText(ownerCommand, "self.provider_registry.execute_authorize", "owner authorize provider execution");
requireText(ownerCommand, "self.provider_registry.execute_capture", "owner capture provider execution");
requireText(ownerCommand, "self.provider_registry.execute_cancel", "owner cancel provider execution");

for (const key of [
  'format!("payment_collection:{}:authorize", collection.id)',
  'format!("payment_collection:{}:capture", collection.id)',
  'format!("payment_collection:{}:cancel", collection.id)',
]) {
  requireText(ownerCommand, key, `canonical provider journal key ${key}`);
}
for (const operation of [
  '"operation": "authorize_payment_collection"',
  '"operation": "capture_payment_collection"',
  '"operation": "cancel_payment_collection"',
]) {
  requireText(ownerCommand, operation, `legacy provider request metadata ${operation}`);
}
requireText(ownerCommand, 'format!("payment_collection:{}:authorize", request.collection_id)', "durable authorize identity recovery");
requireText(ownerCommand, "PROVIDER_OPERATION_RECONCILIATION_REQUIRED", "reconciliation adoption");
requireText(ownerCommand, ".mark_reconciliation_required(", "reconciliation checkpointing");
requireText(ownerCommand, "payment provider outcome requires reconciliation", "bounded reconciliation error");
forbidText(ownerCommand, "source.to_string()", "raw provider error persistence");
forbidText(ownerCommand, "error.to_string()", "raw owner error persistence");

for (const call of [
  ".authorize_payment_collection(",
  ".capture_payment_collection(",
  ".cancel_payment_collection(",
]) {
  requireText(mounted, ".payment_admin_collection_command_port()", "mounted Payment command runtime access");
  requireText(mounted, call, `mounted Payment owner command ${call}`);
}
requireText(mounted, "Permission::PAYMENTS_UPDATE", "payment update permission preservation");
requireText(mounted, 'format!("admin-payment-collection:{collection_id}:{operation}")', "stable transport write identity");
requireText(mounted, ".with_deadline(std::time::Duration::from_secs(2))", "bounded command deadline");
requireText(mounted, "request_context.channel_slug.as_deref()", "channel propagation");
forbidText(mounted, "PaymentOrchestrationService::new", "Commerce orchestration construction in mounted adapter");
forbidText(mounted, "PaymentService::new", "concrete Payment service construction in mounted adapter");

requireText(httpRuntime, "payment_admin_collection_command_runtime: rustok_payment::PaymentAdminCollectionCommandRuntime", "Commerce command runtime field");
requireText(httpRuntime, ".shared_get::<rustok_payment::PaymentAdminCollectionCommandRuntime>()", "host-selected command runtime preference");
requireText(httpRuntime, "rustok_payment::PaymentAdminCollectionCommandRuntime::in_process(", "built-in owner command fallback");
requireText(httpRuntime, "payment_provider_registry.clone()", "host provider registry reuse");

for (const refundMutation of ["create_refund", "complete_refund", "cancel_refund"]) {
  requireText(legacy, `pub async fn ${refundMutation}`, `legacy refund mutation ${refundMutation}`);
}
requireText(
  plan,
  "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.",
  "canonical topology item remains open",
);
requireText(doc, "Refund creation/completion/cancellation are intentionally out of scope", "refund follow-up remains explicit");
requireText(doc, "execution evidence pending and unvalidated", "unvalidated source status");
requireText(doc, "not a newly claimed durable command receipt", "transport versus durable replay distinction");

console.log("commerce admin Payment collection owner-command cutover source guard: OK");
