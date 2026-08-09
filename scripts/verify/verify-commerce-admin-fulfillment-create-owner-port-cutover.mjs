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
const orchestration = read("crates/rustok-commerce/src/services/admin_manual_fulfillment_orchestration.rs");
const servicesMod = read("crates/rustok-commerce/src/services/mod.rs");
const httpRuntime = read("crates/rustok-commerce/src/controllers/mod.rs");
const fulfillmentLib = read("crates/rustok-fulfillment/src/lib.rs");
const ownerCreate = read("crates/rustok-fulfillment/src/admin_create_command.rs");
const plan = read("crates/rustok-commerce/docs/implementation-plan.md");
const doc = read("crates/rustok-commerce/docs/admin-fulfillment-create-owner-port-cutover-2026-08-09.md");

requireText(adminMod, '#[path = "fulfillments.rs"]\nmod fulfillments_legacy;', "private legacy Fulfillment module");
requireText(adminMod, '#[path = "fulfillments_owner_commands.rs"]\npub mod fulfillments;', "mounted Fulfillment owner adapter");
requireText(mounted, "pub use super::fulfillments_legacy::*;", "legacy/Utoipa compatibility re-export");
requireText(mounted, "pub async fn create_fulfillment", "mounted create handler");
requireText(mounted, "Permission::FULFILLMENTS_CREATE", "create permission");
requireText(mounted, "AdminManualFulfillmentOrchestrationService::new(", "typed cross-owner orchestration");
requireText(mounted, "runtime.order_read_port()", "Order owner read capability");
requireText(mounted, "runtime.fulfillment_read_port()", "Fulfillment owner read capability");
requireText(mounted, "runtime.shipping_option_read_port()", "ShippingOption owner read capability");
requireText(mounted, "runtime.fulfillment_admin_create_command_port()", "Fulfillment owner create capability");
requireText(mounted, '"admin-fulfillment:create:{}:{first:016x}{second:016x}"', "input-sensitive transport write identity");
requireText(mounted, "0xcbf29ce484222325", "first admission FNV offset");
requireText(mounted, "0x84222325cbf29ce4", "second admission FNV offset");
requireText(mounted, ".with_deadline(std::time::Duration::from_secs(2))", "bounded read/write deadline");
requireText(mounted, "request_context.channel_slug.as_deref()", "channel propagation");
requireText(mounted, '"commerce_admin_fulfillment_reconciliation_required"', "reconciliation public envelope");
forbidText(mounted, "FulfillmentService::new", "concrete Fulfillment construction in mounted create path");
forbidText(mounted, "FulfillmentOrchestrationService::new", "legacy Commerce orchestration construction in mounted create path");
forbidText(mounted, "runtime.db_clone()", "direct Commerce DB access in mounted create adapter");
forbidText(mounted, "rustok_order::entities", "Order ORM access in mounted create adapter");

requireText(servicesMod, "mod admin_manual_fulfillment_orchestration;", "manual fulfillment orchestration module");
requireText(servicesMod, "AdminManualFulfillmentOrchestrationService", "manual fulfillment orchestration export");
requireText(orchestration, "Arc<dyn OrderReadPort>", "Order owner read port field");
requireText(orchestration, "Arc<dyn FulfillmentReadPort>", "Fulfillment owner read port field");
requireText(orchestration, "Arc<dyn ShippingOptionReadPort>", "ShippingOption owner read port field");
requireText(orchestration, "Arc<dyn FulfillmentAdminCreateCommandPort>", "Fulfillment owner create port field");
requireText(orchestration, ".read_order_projection(", "Order projection read");
requireText(orchestration, ".list_fulfillment_projections(", "existing Fulfillment projection reads");
requireText(orchestration, ".read_shipping_option_projection(", "ShippingOption projection read");
requireText(orchestration, ".create_fulfillment(", "owner create command call");
requireText(orchestration, "is_shipping_option_compatible_with_profiles", "existing shipping-profile compatibility policy");
requireText(orchestration, '"post_order": {\n                            "manual": true', "manual fulfillment item metadata");
requireText(orchestration, '"delivery_group"', "delivery-group metadata");
forbidText(orchestration, "sea_orm", "SeaORM in cross-owner orchestration");
forbidText(orchestration, "rustok_order::entities", "Order ORM in cross-owner orchestration");
forbidText(orchestration, "FulfillmentService", "concrete Fulfillment service in cross-owner orchestration");

requireText(fulfillmentLib, "mod admin_create_command;", "Fulfillment admin create module");
requireText(fulfillmentLib, "FulfillmentAdminCreateCommandPort", "Fulfillment create port export");
requireText(fulfillmentLib, "FulfillmentAdminCreateCommandRuntime", "Fulfillment create runtime export");
requireText(ownerCreate, "pub trait FulfillmentAdminCreateCommandPort", "Fulfillment create owner trait");
requireText(ownerCreate, "pub struct FulfillmentAdminCreateCommandRuntime", "Fulfillment create owner runtime");
requireText(ownerCreate, "context.require_policy(PortCallPolicy::write())", "owner write admission");
requireText(ownerCreate, "FulfillmentService::new(db.clone())", "owner-local Fulfillment service");
requireText(ownerCreate, "FulfillmentProviderOperationJournal::new(db)", "owner-local provider journal");
requireText(ownerCreate, ".execute_create_label(provider_id, request)", "owner create-label provider execution");
requireText(ownerCreate, 'format!("fulfillment:{}:create_label", fulfillment.id)', "exact durable create-label identity");
requireText(ownerCreate, 'operation: "create_label".to_string()', "create-label journal operation");
requireText(ownerCreate, '"operation": "create_label"', "create-label provider metadata");
requireText(ownerCreate, "PROVIDER_OPERATION_COMMITTED", "committed replay adoption");
requireText(ownerCreate, "PROVIDER_OPERATION_SUCCEEDED", "provider-succeeded replay adoption");
requireText(ownerCreate, "PROVIDER_OPERATION_RECONCILIATION_REQUIRED", "reconciliation replay handling");
requireText(ownerCreate, "mark_execution_reconciliation_required", "provider-result serialization reconciliation checkpoint");
requireText(ownerCreate, '"fulfillment.reconciliation_required"', "bounded post-persistence reconciliation error");
forbidText(ownerCreate, "rustok_commerce", "Commerce dependency in Fulfillment owner create");
forbidText(ownerCreate, "rustok_order", "Order dependency in Fulfillment owner create");
forbidText(ownerCreate, "source.to_string()", "raw source persistence in Fulfillment owner create");
forbidText(ownerCreate, "error.to_string()", "raw error persistence in Fulfillment owner create");

requireText(httpRuntime, "fulfillment_admin_create_command_runtime:", "Commerce Fulfillment create runtime field");
requireText(httpRuntime, "fn fulfillment_admin_create_command_port(", "Commerce Fulfillment create port accessor");
requireText(httpRuntime, ".shared_get::<rustok_fulfillment::FulfillmentAdminCreateCommandRuntime>()", "host-selected create runtime preference");
requireText(httpRuntime, "rustok_fulfillment::FulfillmentAdminCreateCommandRuntime::in_process(", "built-in create owner fallback");
requireText(httpRuntime, "fulfillment_provider_registry.clone()", "host-selected Fulfillment provider registry reuse");

requireText(legacy, "pub async fn create_fulfillment", "legacy create compatibility source retained");
requireText(legacy, "FulfillmentOrchestrationService::new(runtime.db_clone())", "legacy compatibility orchestration retained");

requireText(plan, "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,", "canonical topology item remains open");
requireText(plan, "Payment, and Fulfillment concrete services behind host-composed owner ports.", "canonical topology continuation remains open");
requireText(doc, "Source-complete for the mounted `POST /admin/fulfillments` route", "focused source-complete status");
requireText(doc, "It is **not** a newly claimed durable manual-fulfillment creation receipt", "transport/durable replay distinction");
requireText(doc, "canonical broad Commerce topology P0 remains open", "broad topology remains open");
requireText(doc, "Execution evidence remains pending and unvalidated", "execution evidence remains open");

console.log("commerce admin Fulfillment create owner-port cutover source guard: OK");
