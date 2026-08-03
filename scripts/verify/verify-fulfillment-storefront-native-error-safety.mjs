#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const sourcePath =
  "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs";
const evidencePath =
  "crates/rustok-fulfillment/contracts/evidence/storefront-native-error-safety-source.json";
const docPath = "crates/rustok-fulfillment/docs/storefront-native-error-safety.md";
const cargo = read("crates/rustok-fulfillment/storefront/Cargo.toml");
const source = read(sourcePath);
const evidence = JSON.parse(read(evidencePath));
const doc = read(docPath);

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

function functionBody(text, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(text);
  if (!match) {
    failures.push(`${sourcePath}: missing function ${functionName}`);
    return "";
  }
  const openBrace = text.indexOf("{", match.index);
  if (openBrace === -1) {
    failures.push(`${sourcePath}: missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < text.length; index += 1) {
    if (text[index] === "{") depth += 1;
    if (text[index] === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(openBrace, index + 1);
    }
  }
  failures.push(`${sourcePath}: unterminated body for ${functionName}`);
  return "";
}

requireText(cargo, "tracing.workspace = true", "fulfillment storefront diagnostics dependency");

for (const [value, label] of [
  ["const FULFILLMENT_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const FULFILLMENT_STOREFRONT_NATIVE_OPERATION", "native operation constant"],
  ["const FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn map_runtime_dependency_error(", "runtime dependency mapper"],
  ["fn map_tenant_context_error<E>(", "tenant context mapper"],
  ["fn map_auth_context_error<E>(", "auth context mapper"],
  ["fn record_optional_request_context_error<E>(", "optional request context logger"],
  ["fn map_owner_runtime_error<E>(", "owner runtime mapper"],
  ["owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ["owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION", "operation diagnostics"],
  ["boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
]) requireText(source, value, label);

for (const obsolete of [
  "fn map_tenant_context_error<E: std::fmt::Debug>(",
  "fn map_auth_context_error<E: std::fmt::Debug>(",
  "fn record_optional_request_context_error<E: std::fmt::Debug>(",
  "fn map_owner_runtime_error<E: std::fmt::Debug>(",
]) forbidText(source, obsolete, "obsolete fulfillment storefront diagnostic contract");

for (const functionName of [
  "map_tenant_context_error",
  "map_auth_context_error",
  "record_optional_request_context_error",
  "map_owner_runtime_error",
]) {
  const body = functionBody(source, functionName);
  requireText(
    body,
    "let error_type = std::any::type_name::<E>();",
    `${functionName} bounded error type`,
  );
  requireText(body, "error_type", `${functionName} error type diagnostic`);
  for (const forbidden of [
    "error = ?error",
    "error = %error",
    "error = ?_error",
    "error = %_error",
  ]) forbidText(body, forbidden, `${functionName} complete error payload`);
}

if (countText(source, "let error_type = std::any::type_name::<E>();") !== 4) {
  failures.push("expected exactly four type-only fulfillment storefront diagnostic sites");
}
if (countText(source, "tenant_id_non_nil = !tenant_id.is_nil()") !== 4) {
  failures.push("expected bounded tenant identity facts in auth, optional-request, and both owner branches");
}
if (countText(source, "request_context_present = true") !== 1) {
  failures.push("owner diagnostics must record one successful request-context presence fact");
}
if (countText(source, "request_context_present = false") !== 2) {
  failures.push("optional extraction and owner fallback must record absent request context");
}
for (const marker of [
  "correlation_id = %request_context.correlation_id",
  "channel_id_present = request_context.channel_id.is_some()",
  "channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil())",
  "channel_slug_present = request_context.channel_slug.is_some()",
  "channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count())",
  "locale_present = !request_context.locale.trim().is_empty()",
  "locale_length = request_context.locale.chars().count()",
]) {
  if (countText(source, marker) !== 1) {
    failures.push(`expected exactly one bounded owner request-context site for ${marker}`);
  }
}

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
  "error = ?error",
  "error = %error",
  "tenant_id = %tenant_id",
  "channel_id = ?request_context.channel_id",
  "channel_slug = ?request_context.channel_slug",
  "locale = %request_context.locale",
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  ".map_err(|error| ServerFnError::new(error.to_string()))",
  "fulfillment/select-shipping-option requires TransactionalEventBus in host runtime context",
]) forbidText(source, value, "unsafe fulfillment storefront native diagnostic or public mapping");

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
  framework_error_type_only: true,
  optional_request_context_error_type_only: true,
  owner_runtime_error_type_only: true,
  complete_internal_error_logged: false,
  correlation_logging_when_available: true,
  tenant_identity_shape_only: true,
  channel_context_shape_only_when_available: true,
  locale_shape_only_when_available: true,
  raw_tenant_channel_locale_logged: false,
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
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
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

requireText(doc, "Status: **source-ready / unvalidated**", "documentation status");
requireText(doc, "complete framework and owner errors are not logged", "documentation error policy");
requireText(doc, "tenant and request-context identity values are not logged", "documentation identity policy");

if (failures.length > 0) {
  console.error("Fulfillment storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ fulfillment storefront native diagnostics use bounded type and request-shape facts with static public envelopes; runtime evidence remains open",
);
