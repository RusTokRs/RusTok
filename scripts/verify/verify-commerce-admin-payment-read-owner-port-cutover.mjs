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

const adminMod = read("crates/rustok-commerce/src/controllers/admin/mod.rs");
const mounted = read("crates/rustok-commerce/src/controllers/admin/payments_owner_reads.rs");
const legacy = read("crates/rustok-commerce/src/controllers/admin/payments.rs");
const httpRuntime = read("crates/rustok-commerce/src/controllers/mod.rs");
const paymentLib = read("crates/rustok-payment/src/lib.rs");
const ownerRead = read("crates/rustok-payment/src/admin_read.rs");
const openapi = read("crates/rustok-commerce/src/openapi.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");

requireText(adminMod, '#[path = "payments.rs"]\nmod payments_legacy;', "legacy payment module alias");
requireText(adminMod, '#[path = "payments_owner_reads.rs"]\npub mod payments;', "mounted payment owner read module");

requireText(paymentLib, "mod admin_read;", "Payment admin read module registration");
requireText(paymentLib, "PaymentAdminReadPort, PaymentAdminReadRuntime", "Payment admin read exports");
requireText(ownerRead, "pub trait PaymentAdminReadPort", "Payment admin read owner port");
requireText(ownerRead, "pub struct PaymentAdminReadRuntime", "Payment admin read runtime");
requireText(ownerRead, "context.require_policy(PortCallPolicy::read())", "read deadline policy admission");
requireText(ownerRead, "PaymentService::new(db)", "owner-local in-process service construction");
requireText(ownerRead, ".list_collections(", "owner collection list delegation");
requireText(ownerRead, ".get_collection(", "owner collection detail delegation");
requireText(ownerRead, ".list_refunds(", "owner refund list delegation");
requireText(ownerRead, ".get_refund(", "owner refund detail delegation");

requireText(mounted, "pub use super::payments_legacy::*;", "legacy mutation/OpenAPI compatibility re-export");
requireText(mounted, "runtime\n        .payment_admin_read_port()", "mounted owner read runtime call");
requireText(mounted, ".list_payment_collection_projections(", "mounted collection list owner call");
requireText(mounted, ".read_payment_collection_projection(", "mounted collection detail owner call");
requireText(mounted, ".list_refund_projections(", "mounted refund list owner call");
requireText(mounted, ".read_refund_projection(", "mounted refund detail owner call");
requireText(mounted, "RequestContext", "trusted request context extractor");
requireText(mounted, ".with_deadline(std::time::Duration::from_secs(2))", "bounded read deadline");
requireText(mounted, "request_context.channel_slug.as_deref()", "resolved channel propagation");
requireText(mounted, "Permission::PAYMENTS_READ", "read permission preservation");
forbidText(mounted, "PaymentService", "concrete Payment service in mounted read adapter");
forbidText(mounted, "runtime.db_clone()", "Commerce DB access in mounted read adapter");

for (const mutation of [
  "authorize_payment_collection",
  "capture_payment_collection",
  "cancel_payment_collection",
  "create_refund",
  "complete_refund",
  "cancel_refund",
]) {
  requireText(legacy, `pub async fn ${mutation}`, `legacy mutation source ${mutation}`);
}

requireText(httpRuntime, "payment_admin_read_runtime: rustok_payment::PaymentAdminReadRuntime", "Commerce payment admin read runtime field");
requireText(httpRuntime, ".shared_get::<rustok_payment::PaymentAdminReadRuntime>()", "host-selected payment admin read runtime");
requireText(httpRuntime, "rustok_payment::PaymentAdminReadRuntime::in_process(runtime.db_clone())", "in-process owner runtime fallback");
requireText(httpRuntime, "fn payment_admin_read_port", "Commerce payment admin read port accessor");

for (const operation of [
  "crate::controllers::admin::list_payment_collections",
  "crate::controllers::admin::show_payment_collection",
  "crate::controllers::admin::list_refunds",
  "crate::controllers::admin::show_refund",
]) {
  requireText(openapi, operation, `OpenAPI root operation ${operation}`);
}

requireText(
  plan,
  "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.",
  "canonical broad topology item remains open",
);

console.log("commerce admin Payment read owner-port cutover source guard: OK");
