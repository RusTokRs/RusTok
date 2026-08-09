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

const adminMod = read("crates/rustok-commerce/src/controllers/admin/mod.rs");
const mounted = read("crates/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs");
const legacy = read("crates/rustok-commerce/src/controllers/admin/fulfillments.rs");
const httpRuntime = read("crates/rustok-commerce/src/controllers/mod.rs");
const fulfillmentLib = read("crates/rustok-fulfillment/src/lib.rs");
const ownerCommand = read("crates/rustok-fulfillment/src/admin_command.rs");
const openapi = read("crates/rustok-commerce/src/openapi.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");
const doc = read("crates/rustok-commerce/docs/admin-fulfillment-owner-command-cutover-2026-08-09.md");

requireText(adminMod, '#[path = "fulfillments.rs"]\nmod fulfillments_legacy;', "private legacy Fulfillment module");
requireText(adminMod, '#[path = "fulfillments_owner_commands.rs"]\npub mod fulfillments;', "mounted Fulfillment owner adapter");
requireText(mounted, "pub use super::fulfillments_legacy::*;", "legacy/Utoipa compatibility re-export");

requireText(fulfillmentLib, "mod admin_command;", "Fulfillment admin command module");
requireText(fulfillmentLib, "FulfillmentAdminCommandPort", "Fulfillment admin command export");
requireText(fulfillmentLib, "FulfillmentAdminCommandRuntime", "Fulfillment admin runtime export");
requireText(ownerCommand, "pub trait FulfillmentAdminCommandPort", "Fulfillment owner command trait");
requireText(ownerCommand, "pub struct FulfillmentAdminCommandRuntime", "Fulfillment owner command runtime");
requireText(ownerCommand, "context.require_policy(PortCallPolicy::write())", "write policy admission");
requireText(ownerCommand, "context.require_write_semantics()", "write semantics admission");
requireText(ownerCommand, "FulfillmentService::new(db.clone())", "owner-local Fulfillment service construction");
requireText(ownerCommand, "FulfillmentProviderOperationJournal::new(db)", "owner-local provider journal construction");
requireText(ownerCommand, "self.provider_registry.execute_ship", "owner ship provider execution");
requireText(ownerCommand, "self.provider_registry.execute_cancel", "owner cancel provider execution");
requireText(ownerCommand, '"fulfillment:{fulfillment_id}:{operation}:{first:016x}{second:016x}"', "canonical payload-sensitive provider key");
requireText(ownerCommand, "0xcbf29ce484222325", "first legacy FNV offset basis");
requireText(ownerCommand, "0x84222325cbf29ce4", "second legacy FNV offset basis");
requireText(ownerCommand, '"operation": "ship"', "ship provider metadata");
requireText(ownerCommand, '"operation": "reship"', "reship provider metadata");
requireText(ownerCommand, '"operation": "cancel"', "cancel provider metadata");
requireText(ownerCommand, '== Some("reship")', "reship replay shortcut");
requireText(ownerCommand, 'if current.status == "cancelled"', "cancel replay shortcut");
requireText(ownerCommand, '"fulfillment.reconciliation_required"', "bounded reconciliation error");
forbidText(ownerCommand, "source.to_string()", "raw provider error persistence");

for (const operation of ["ship", "deliver", "reopen", "reship", "cancel"]) {
  requireText(mounted, `pub async fn ${operation}_fulfillment`, `mounted ${operation} handler`);
  requireText(mounted, `.fulfillment_admin_command_port()`, "mounted Fulfillment command runtime access");
  requireText(mounted, `.${operation}_fulfillment(`, `mounted owner ${operation} call`);
}
requireText(mounted, "Permission::FULFILLMENTS_UPDATE", "Fulfillment update permission");
requireText(mounted, 'format!("admin-fulfillment:{fulfillment_id}:{operation}")', "stable transport write identity");
requireText(mounted, ".with_deadline(std::time::Duration::from_secs(2))", "bounded command deadline");
requireText(mounted, "request_context.channel_slug.as_deref()", "channel propagation");
requireText(mounted, '"commerce_admin_fulfillment_reconciliation_required"', "reconciliation public envelope");
forbidText(mounted, "FulfillmentService::new", "concrete Fulfillment construction in mounted adapter");
forbidText(mounted, "FulfillmentOrchestrationService::new", "Commerce orchestration construction for mounted five commands");
forbidText(mounted, "runtime.db_clone()", "direct Commerce DB access in mounted command adapter");

requireText(legacy, "pub async fn create_fulfillment", "cross-owner create remains compatibility source");
requireText(legacy, "FulfillmentOrchestrationService::new(runtime.db_clone())", "manual create remains cross-owner orchestration");

requireText(httpRuntime, "fulfillment_admin_command_runtime: rustok_fulfillment::FulfillmentAdminCommandRuntime", "Commerce Fulfillment command runtime field");
requireText(httpRuntime, "fn fulfillment_admin_command_port(", "Commerce Fulfillment command accessor");
requireText(httpRuntime, ".shared_get::<rustok_fulfillment::FulfillmentAdminCommandRuntime>()", "host-selected Fulfillment command runtime preference");
requireText(httpRuntime, "rustok_fulfillment::FulfillmentAdminCommandRuntime::in_process(", "built-in Fulfillment owner fallback");
requireText(httpRuntime, "fulfillment_provider_registry.clone()", "host Fulfillment provider registry reuse");

for (const symbol of [
  "ship_fulfillment",
  "deliver_fulfillment",
  "reopen_fulfillment",
  "reship_fulfillment",
  "cancel_fulfillment",
]) {
  requireText(openapi, `crate::controllers::admin::${symbol}`, `OpenAPI root symbol ${symbol}`);
}

requireText(plan, "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,", "canonical topology item remains open");
requireText(plan, "Payment, and Fulfillment concrete services behind host-composed owner ports.", "canonical topology continuation remains open");
requireText(doc, "Manual fulfillment creation is a cross-owner Commerce workflow", "create follow-up boundary");
requireText(doc, "execution evidence pending and unvalidated", "unvalidated source status");
requireText(doc, "It is not a newly claimed durable command receipt", "transport/durable replay distinction");

console.log("commerce admin Fulfillment owner-command cutover source guard: OK");
