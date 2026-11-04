#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { containsMarker } from "./lib/source-text.mjs";

const root = process.cwd();
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const requireText = (source, text, label) => {
  if (!containsMarker(source, text))
    throw new Error(`missing ${label}: ${text}`);
};
const forbidText = (source, text, label) => {
  if (containsMarker(source, text))
    throw new Error(`forbidden ${label}: ${text}`);
};

const adminMod = read(
  "crates/modules/rustok-commerce/src/controllers/admin/mod.rs",
);
const mounted = read(
  "crates/modules/rustok-commerce/src/controllers/admin/fulfillments_owner_commands.rs",
);
const httpRuntime = read(
  "crates/modules/rustok-commerce/src/controllers/mod.rs",
);
const fulfillmentLib = read("crates/modules/rustok-fulfillment/src/lib.rs");
const ownerCommand = read(
  "crates/modules/rustok-fulfillment/src/admin_command.rs",
);
const openapi = read("crates/modules/rustok-commerce/src/openapi.rs");
const plan = read("crates/modules/rustok-commerce/docs/implementation-plan.md");


requireText(
  adminMod,
  '#[path = "fulfillments_owner_commands.rs"]\npub mod fulfillments;',
  "mounted Fulfillment owner adapter",
);

requireText(
  fulfillmentLib,
  "mod admin_command;",
  "Fulfillment admin command module",
);
requireText(
  fulfillmentLib,
  "FulfillmentAdminCommandPort",
  "Fulfillment admin command export",
);
requireText(
  fulfillmentLib,
  "FulfillmentAdminCommandRuntime",
  "Fulfillment admin runtime export",
);
requireText(
  ownerCommand,
  "pub trait FulfillmentAdminCommandPort",
  "Fulfillment owner command trait",
);
requireText(
  ownerCommand,
  "pub struct FulfillmentAdminCommandRuntime",
  "Fulfillment owner command runtime",
);
requireText(
  ownerCommand,
  "context.require_policy(PortCallPolicy::write())",
  "write policy admission",
);
requireText(
  ownerCommand,
  "context.require_write_semantics()",
  "write semantics admission",
);
requireText(
  ownerCommand,
  "FulfillmentService::new(db.clone())",
  "owner-local Fulfillment service construction",
);
requireText(
  ownerCommand,
  "FulfillmentProviderOperationJournal::new(db)",
  "owner-local provider journal construction",
);
requireText(
  ownerCommand,
  "self.provider_registry.execute_ship",
  "owner ship provider execution",
);
requireText(
  ownerCommand,
  "self.provider_registry.execute_cancel",
  "owner cancel provider execution",
);
requireText(ownerCommand, '"operation": "ship"', "ship provider metadata");
requireText(ownerCommand, '"operation": "reship"', "reship provider metadata");
requireText(ownerCommand, '"operation": "cancel"', "cancel provider metadata");
requireText(
  ownerCommand,
  "let idempotency_key = context",
  "canonical caller key admission",
);
requireText(
  ownerCommand,
  "idempotency_key: Some(idempotency_key.to_string())",
  "literal provider identity",
);
requireText(
  ownerCommand,
  "PROVIDER_OPERATION_COMMITTED",
  "committed journal replay",
);
requireText(
  ownerCommand,
  "execute_progress_command",
  "kernel progress receipt boundary",
);
forbidText(
  ownerCommand,
  "stable_operation_key(",
  "no reconstructed lifecycle identity",
);
requireText(
  ownerCommand,
  '"fulfillment.reconciliation_required"',
  "bounded reconciliation error",
);
forbidText(
  ownerCommand,
  "source.to_string()",
  "raw provider error persistence",
);

for (const operation of ["ship", "deliver", "reopen", "reship", "cancel"]) {
  requireText(
    mounted,
    `pub async fn ${operation}_fulfillment`,
    `mounted ${operation} handler`,
  );
  requireText(
    mounted,
    `.fulfillment_admin_command_port()`,
    "mounted Fulfillment command runtime access",
  );
  requireText(
    mounted,
    `.${operation}_fulfillment(`,
    `mounted owner ${operation} call`,
  );
}
forbidText(
  adminMod,
  "mod fulfillments_legacy",
  "no obsolete parallel controller",
);
forbidText(
  mounted,
  "pub use super::fulfillments_legacy",
  "no wildcard legacy handler fallback",
);
requireText(mounted, "auth.port_actor()", "canonical human/service principal");
requireText(
  mounted,
  "request_context.correlation_id.clone()",
  "actual request correlation",
);
requireText(
  mounted,
  "auth.scopes.iter().cloned()",
  "actual permission/scope claims",
);
requireText(
  mounted,
  "let idempotency_key = require_idempotency_key(&headers)?;",
  "caller-owned HTTP write identity",
);
requireText(
  mounted,
  "value.is_empty() || value.len() > 191",
  "bounded caller key",
);
requireText(
  mounted,
  "Permission::FULFILLMENTS_UPDATE",
  "Fulfillment update permission",
);
requireText(
  mounted,
  ".with_deadline(std::time::Duration::from_secs(2))",
  "bounded command deadline",
);
requireText(
  mounted,
  "request_context.channel_slug.as_deref()",
  "channel propagation",
);
requireText(
  mounted,
  '"commerce_admin_fulfillment_reconciliation_required"',
  "reconciliation public envelope",
);
forbidText(
  mounted,
  "FulfillmentService::new",
  "concrete Fulfillment construction in mounted adapter",
);
forbidText(
  mounted,
  /\bFulfillmentOrchestrationService::new\s*\(/,
  "Commerce orchestration construction for mounted five commands",
);
forbidText(
  mounted,
  "runtime.db_clone()",
  "direct Commerce DB access in mounted command adapter",
);

requireText(
  httpRuntime,
  "fulfillment_admin_command_runtime: rustok_fulfillment::FulfillmentAdminCommandRuntime",
  "Commerce Fulfillment command runtime field",
);
requireText(
  httpRuntime,
  "fn fulfillment_admin_command_port(",
  "Commerce Fulfillment command accessor",
);
requireText(
  httpRuntime,
  ".shared_get::<rustok_fulfillment::FulfillmentAdminCommandRuntime>()",
  "host-selected Fulfillment command runtime preference",
);
requireText(
  httpRuntime,
  "crate::fulfillment_admin_command_runtime(",
  "built-in Fulfillment owner fallback with the inventory-backed stock port",
);
requireText(
  httpRuntime,
  "fulfillment_provider_registry.clone()",
  "host Fulfillment provider registry reuse",
);

for (const symbol of [
  "ship_fulfillment",
  "deliver_fulfillment",
  "reopen_fulfillment",
  "reship_fulfillment",
  "cancel_fulfillment",
]) {
  requireText(
    openapi,
    `crate::controllers::admin::${symbol}`,
    `OpenAPI root symbol ${symbol}`,
  );
}

requireText(
  plan,
  "- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,",
  "canonical topology item remains open",
);
requireText(
  plan,
  "Payment, and Fulfillment concrete services behind host-composed owner ports.",
  "canonical topology continuation remains open",
);

console.log(
  "commerce admin Fulfillment owner-command cutover source guard: OK",
);
