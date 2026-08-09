#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const requireText = (source, text, label) => {
  if (!source.includes(text)) throw new Error(`missing ${label}: ${text}`);
};
const forbidText = (source, text, label) => {
  if (source.includes(text)) throw new Error(`forbidden ${label}: ${text}`);
};

const mounted = read("crates/rustok-commerce/src/controllers/admin/payments_owner_reads.rs");
const adminModule = read("crates/rustok-commerce/src/controllers/admin/mod.rs");
const httpRuntime = read("crates/rustok-commerce/src/controllers/mod.rs");
const paymentLib = read("crates/rustok-payment/src/lib.rs");
const owner = read("crates/rustok-payment/src/admin_refund_command.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");
const doc = read("crates/rustok-commerce/docs/admin-refund-owner-command-cutover-2026-08-09.md");

requireText(paymentLib, "mod admin_refund_command;", "Payment refund command module");
requireText(paymentLib, "PaymentAdminRefundCommandPort", "Payment refund command port export");
requireText(paymentLib, "PaymentAdminRefundCommandRuntime", "Payment refund command runtime export");
requireText(owner, "pub trait PaymentAdminRefundCommandPort", "owner refund command trait");
requireText(owner, "PaymentRefundCreationService::new(db.clone())", "owner-local refund reservation service");
requireText(owner, "PaymentProviderOperationJournal::new(db)", "owner-local provider journal");
requireText(owner, "context.require_policy(PortCallPolicy::write())", "write admission");
requireText(owner, ".create_or_replay(", "durable refund reservation replay");
requireText(owner, 'idempotency_key: Some(format!("payment_refund:{}", refund.id))', "stable provider refund identity");
requireText(owner, 'operation: "refund".to_string()', "provider refund operation");
requireText(owner, "refund_id: Some(refund_id)", "provider journal refund relation");
requireText(owner, '"operation": "create_refund"', "legacy provider metadata operation");
requireText(owner, 'format!("payment_collection:{}:authorize", request.collection_id)', "authorize provider identity recovery");
requireText(owner, "self.provider_registry.execute_refund", "Payment-owned provider execution");
requireText(owner, "payment.refund_reserved_reconciliation_required", "reserved refund reconciliation outcome");
requireText(owner, "payment.refund_reserved_provider_unavailable", "reserved refund unavailable outcome");
requireText(owner, ".complete_refund(tenant_id, request.refund_id, request.input)", "owner refund completion");
requireText(owner, ".cancel_refund(tenant_id, request.refund_id, request.input)", "owner refund cancellation");
forbidText(owner, "source.to_string()", "raw provider source persistence");
forbidText(owner, "error.to_string()", "raw owner error persistence");

for (const symbol of ["create_refund", "complete_refund", "cancel_refund"]) {
  requireText(mounted, `pub async fn ${symbol}`, `mounted ${symbol} handler`);
}
requireText(mounted, ".payment_admin_refund_command_port()", "mounted refund owner runtime access");
requireText(mounted, "Permission::PAYMENTS_UPDATE", "refund update permission");
requireText(mounted, 'headers.get("idempotency-key")', "caller refund idempotency header");
requireText(mounted, "MAX_REFUND_CREATION_KEY_LENGTH: usize = 191", "refund creation key bound");
requireText(mounted, ".with_idempotency_key(creation_key.to_string())", "caller key write admission propagation");
requireText(mounted, ".with_deadline(std::time::Duration::from_secs(2))", "bounded refund command deadline");
requireText(mounted, "request_context.channel_slug.as_deref()", "refund channel propagation");
requireText(mounted, "payment.refund_reserved_reconciliation_required", "refund-specific reconciliation HTTP mapping");
requireText(mounted, "commerce_admin_refund_reconciliation_required", "public refund reconciliation envelope");
requireText(mounted, "commerce_admin_refund_provider_unavailable", "public reserved refund unavailable envelope");
forbidText(mounted, "PaymentOrchestrationService::new", "mounted Commerce Payment orchestration construction");
forbidText(mounted, "PaymentService::new", "mounted concrete Payment service construction");

requireText(adminModule, '#[path = "payments_owner_reads.rs"]\npub mod payments;', "owner-mounted admin Payment module");
requireText(httpRuntime, "payment_admin_refund_command_runtime: rustok_payment::PaymentAdminRefundCommandRuntime", "refund command runtime field");
requireText(httpRuntime, ".shared_get::<rustok_payment::PaymentAdminRefundCommandRuntime>()", "host-selected refund command runtime preference");
requireText(httpRuntime, "rustok_payment::PaymentAdminRefundCommandRuntime::in_process(", "built-in Payment refund owner fallback");
requireText(httpRuntime, "payment_provider_registry.clone()", "host provider registry reuse");

requireText(
  plan,
  "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.",
  "canonical topology item remains open",
);
requireText(doc, "execution evidence pending and unvalidated", "unvalidated source status");
requireText(doc, "payment_refund:{refund_id}", "provider refund identity documentation");
requireText(doc, "two different durable identities", "creation versus provider identity distinction");
requireText(doc, "remains open", "broad topology follow-up");

console.log("commerce admin refund owner-command cutover source guard: OK");
