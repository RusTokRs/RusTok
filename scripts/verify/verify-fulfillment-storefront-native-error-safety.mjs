#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const cargo = read("crates/rustok-fulfillment/storefront/Cargo.toml");
const source = read(
  "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-fulfillment/contracts/evidence/storefront-native-error-safety-source.json",
  ),
);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

requireText(cargo, "tracing.workspace = true", "fulfillment storefront diagnostics dependency");

for (const [value, label] of [
  ["const FULFILLMENT_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const FULFILLMENT_STOREFRONT_NATIVE_OPERATION", "native operation constant"],
  ["const FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn map_tenant_context_error<E: std::fmt::Debug>(", "tenant context mapper"],
  ["fn map_auth_context_error<E: std::fmt::Debug>(", "auth context mapper"],
  ["fn record_optional_request_context_error<E: std::fmt::Debug>(", "optional request context logger"],
  ["fn map_owner_runtime_error<E: std::fmt::Debug>(", "owner runtime mapper"],
  ["owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ["owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION", "operation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["correlation_id = %request_context.correlation_id", "correlation diagnostics"],
  ["channel_id = ?request_context.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request_context.channel_slug", "channel slug diagnostics"],
  ["locale = %request_context.locale", "locale diagnostics"],
  ["boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
  ["error = ?error", "internal cause diagnostics"],
]) requireText(source, value, label);

for (const [value, label] of [
  ["endpoint = \"fulfillment/select-shipping-option\"", "shipping-selection endpoint"],
  ["select_storefront_shipping_option(", "owner runtime call"],
  ["StorefrontShippingSelectionCommand {", "owner command payload"],
  ["cart_id,", "cart command field"],
  ["shipping_selections,", "shipping selections command field"],
  ["request_context.as_ref(),", "optional owner request context"],
  ["build_shipping_selection_updates(&request)", "selection validation seam"],
  ["ServerFnError::new(error.message().to_string())", "selection validation compatibility"],
  ["ServerFnError::new(\"cart_id must be a valid UUID\")", "cart UUID validation compatibility"],
  ["ServerFnError::new(format!(\"{field_name} must be a valid UUID\"))", "option UUID validation compatibility"],
  ["ShippingSelectionTransportError::ServerFn(error.to_string())", "outer transport wrapper"],
]) requireText(source, value, label);

for (const [value, label] of [
  ["fulfillment.storefront_runtime_unavailable", "runtime stable code"],
  ["fulfillment.storefront_tenant_context_unavailable", "tenant stable code"],
  ["fulfillment.storefront_auth_context_unavailable", "auth stable code"],
  ["fulfillment.storefront_request_context_unavailable", "optional request context stable code"],
  ["fulfillment.storefront_shipping_selection_failed", "owner runtime stable code"],
  ["Shipping selection is temporarily unavailable", "shipping selection public message"],
  ["Shipping selection context is unavailable", "shipping context public message"],
]) requireText(source, value, label);

if (countText(source, "ShippingSelectionTransportError::ServerFn(error.to_string())") !== 1) {
  failures.push("the outer native transport wrapper must remain unchanged exactly once");
}
if (countText(source, "ServerFnError::new(\"Shipping selection is temporarily unavailable\")") !== 2) {
  failures.push("runtime and owner failures must share the stable shipping-selection envelope");
}
if (countText(source, "ServerFnError::new(\"Shipping selection context is unavailable\")") !== 2) {
  failures.push("tenant and auth failures must share the stable context envelope");
}
if (countText(source, "request_context.as_ref()") !== 2) {
  failures.push("optional request context must be passed to the owner call and owner error mapper");
}

for (const value of [
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  ".map_err(|error| ServerFnError::new(error.to_string()))",
  "fulfillment/select-shipping-option requires TransactionalEventBus in host runtime context",
]) forbidText(source, value, "raw fulfillment storefront native public mapping");

if (evidence.status !== "fulfillment_storefront_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  runtime_dependency_static_public_envelope: true,
  tenant_context_static_public_envelope: true,
  auth_context_static_public_envelope: true,
  owner_runtime_static_public_envelope: true,
  optional_request_context_preserved: true,
  optional_request_context_failure_logged: true,
  outer_transport_variant_changed: false,
  validation_messages_changed: false,
  graphql_adapter_changed: false,
  request_response_dto_changed: false,
  raw_internal_error_public: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Fulfillment storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ fulfillment storefront native failures retain server diagnostics and static public envelopes; runtime evidence remains open",
);
