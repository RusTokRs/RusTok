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
  "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs";
const source = read(sourcePath);
const evidence = JSON.parse(
  read(
    "crates/rustok-order/contracts/evidence/storefront-runtime-error-diagnostics-source.json",
  ),
);
const doc = read("crates/rustok-order/docs/storefront-runtime-error-diagnostics.md");

const failures = [];
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;

function functionBody(text, functionName) {
  const match = new RegExp(`fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`).exec(text);
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

for (const [value, label] of [
  ["const ORDER_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const ORDER_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn native_context_error<E>(", "context error mapper"],
  ["fn native_checkout_runtime_error(", "runtime error mapper"],
  ["request_context: &rustok_api::RequestContext", "request context mapper input"],
  ["tenant_id: Uuid", "tenant mapper input"],
  ["correlation_id: Uuid", "correlation mapper input"],
  ["let correlation_id = Uuid::new_v4();", "server-generated correlation id"],
  ["owner = ORDER_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ['owner_operation = "complete_storefront_checkout"', "runtime operation diagnostics"],
  ["owner_operation = operation", "context operation diagnostics"],
  ["correlation_id = %correlation_id", "correlation diagnostics"],
  ["tenant_id_non_nil = !tenant_id.is_nil()", "tenant shape diagnostics"],
  ["channel_id_present = request_context.channel_id.is_some()", "channel presence diagnostics"],
  [
    "channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil())",
    "channel non-nil diagnostics",
  ],
  ["channel_slug_present = request_context.channel_slug.is_some()", "channel slug presence"],
  [
    "channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count())",
    "channel slug length",
  ],
  ["locale_present = !request_context.locale.trim().is_empty()", "locale presence"],
  ["locale_length = request_context.locale.chars().count()", "locale length"],
  ["public_code = %public_code", "public code diagnostics"],
  ["let public_retryable = error.retryable();", "retryability source"],
  ["public_retryable,", "retryability diagnostics"],
  ['code = "order.storefront_context_unavailable"', "context stable internal code"],
  ['code = "order.storefront_checkout_runtime_failed"', "runtime stable internal code"],
  ["boundary = ORDER_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
]) requireText(source, value, label);

for (const obsolete of [
  "fn native_context_error(operation: &'static str, error: impl std::fmt::Display)",
  "error = %error",
  "error = ?error",
  "tenant_id = %tenant_id",
  "channel_id = ?request_context.channel_id",
  "channel_slug = ?request_context.channel_slug",
  "locale = %request_context.locale",
]) forbidText(source, obsolete, "obsolete or unsafe Order storefront diagnostic contract");

const contextBody = functionBody(source, "native_context_error");
requireText(
  contextBody,
  "let error_type = std::any::type_name::<E>();",
  "context mapper bounded error type",
);
requireText(contextBody, "error_type", "context mapper error type diagnostic");
for (const forbidden of ["error = ?", "error = %", "_error = ?", "_error = %"]) {
  forbidText(contextBody, forbidden, "context mapper complete error payload");
}

const runtimeBody = functionBody(source, "native_checkout_runtime_error");
requireText(
  runtimeBody,
  "let error_type = std::any::type_name_of_val(&error);",
  "runtime mapper bounded error type",
);
requireText(runtimeBody, "error_type", "runtime mapper error type diagnostic");
for (const forbidden of ["error = ?error", "error = %error"]) {
  forbidText(runtimeBody, forbidden, "runtime mapper complete error payload");
}

if (countText(source, "let error_type = std::any::type_name::<E>();") !== 1) {
  failures.push("expected exactly one type-only context extraction diagnostic site");
}
if (countText(source, "let error_type = std::any::type_name_of_val(&error);") !== 1) {
  failures.push("expected exactly one type-only checkout runtime diagnostic site");
}
for (const marker of [
  "tenant_id_non_nil = !tenant_id.is_nil()",
  "channel_id_present = request_context.channel_id.is_some()",
  "channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil())",
  "channel_slug_present = request_context.channel_slug.is_some()",
  "channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count())",
  "locale_present = !request_context.locale.trim().is_empty()",
  "locale_length = request_context.locale.chars().count()",
]) {
  if (countText(source, marker) !== 1) {
    failures.push(`expected exactly one bounded runtime identity site for ${marker}`);
  }
}

for (const [value, label] of [
  ['endpoint = "order/complete-checkout"', "endpoint"],
  ["shared_get::<TransactionalEventBus>()", "event bus composition"],
  ["shared_get::<PaymentProviderRegistry>()", "payment registry composition"],
  ["shared_get::<ProductCatalogReadRuntime>()", "Product runtime composition"],
  ["extract::<rustok_api::RequestContext>()", "request context extraction"],
  ["extract::<rustok_api::TenantContext>()", "tenant context extraction"],
  ["extract::<rustok_api::OptionalAuthContext>()", "optional auth extraction"],
  ['ServerFnError::new("Checkout request is invalid")', "checkout validation message"],
  ["StorefrontCheckoutCompletionCommand {", "owner command"],
  ['"source_module": metadata.source_module', "source module payload"],
  ['"source_surface": metadata.source_surface', "source surface payload"],
  ['"command": metadata.command', "command metadata payload"],
  ['"owner_module": metadata.owner_module', "owner module payload"],
  ['"create_fulfillment": metadata.create_fulfillment', "fulfillment metadata payload"],
  ["map_checkout_completion(completion)", "completion mapping"],
  ["CheckoutCompletionTransportError::ServerFn(", "outer transport variant"],
  ['"Checkout transport is temporarily unavailable".to_string()', "outer transport message"],
]) requireText(source, value, label);

requireText(
  source,
  "native_checkout_runtime_error(&request_context, tenant.id, correlation_id, error)",
  "server-correlation-aware runtime mapper call",
);
requireText(source, "let public_code = error.public_code();", "public code source");
requireText(source, "let public_message = error.public_message();", "public message source");
requireText(
  source,
  'ServerFnError::new(format!("{public_code}: {public_message}"))',
  "unchanged public code-message envelope",
);

if (countText(source, 'ServerFnError::new("Checkout request is invalid")') !== 2) {
  failures.push("cart id and idempotency validation must preserve two static invalid-request envelopes");
}
if (countText(source, 'ServerFnError::new("Checkout service is temporarily unavailable")') !== 2) {
  failures.push("two required runtime dependencies must preserve their static unavailable envelopes");
}
if (countText(source, 'ServerFnError::new("Checkout request context is unavailable")') !== 1) {
  failures.push("context extraction failures must preserve one static unavailable envelope");
}
forbidText(
  source,
  "request_context.correlation_id",
  "nonexistent RequestContext correlation field",
);
forbidText(
  source,
  "correlation_id = %idempotency_key",
  "idempotency key in diagnostics",
);
forbidText(
  source,
  ".map_err(native_checkout_runtime_error)?;",
  "runtime mapper without request diagnostics",
);

if (evidence.status !== "order_storefront_runtime_error_diagnostics_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  context_error_type_only: true,
  runtime_error_type_only: true,
  complete_internal_error_logged: false,
  owner_operation_logged: true,
  correlation_logged: true,
  correlation_generated_server_side: true,
  request_context_correlation_required: false,
  idempotency_key_logged: false,
  tenant_identity_shape_only: true,
  channel_context_shape_only: true,
  locale_shape_only: true,
  raw_tenant_channel_locale_logged: false,
  stable_code_logged: true,
  boundary_logged: true,
  public_code_retryability_logged: true,
  public_code_message_envelope_preserved: true,
  endpoint_changed: false,
  command_payload_changed: false,
  dependency_composition_changed: false,
  context_extraction_changed: false,
  validation_messages_changed: false,
  outer_transport_envelope_changed: false,
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
requireText(doc, "complete context and runtime errors are not logged", "documentation error policy");
requireText(doc, "tenant and request-context identity values are not logged", "documentation identity policy");

if (failures.length > 0) {
  console.error("Order storefront runtime-error diagnostics verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Order storefront checkout diagnostics use bounded type and request-shape facts while preserving the public envelope; runtime evidence remains open",
);
