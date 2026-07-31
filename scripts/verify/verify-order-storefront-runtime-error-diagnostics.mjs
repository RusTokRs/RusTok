#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL("../../", import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), "utf8");

const source = read(
  "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-order/contracts/evidence/storefront-runtime-error-diagnostics-source.json",
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

for (const [value, label] of [
  ["const ORDER_STOREFRONT_NATIVE_OWNER", "native owner constant"],
  ["const ORDER_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn native_checkout_runtime_error(", "runtime error mapper"],
  ["request_context: &rustok_api::RequestContext", "request context mapper input"],
  ["tenant_id: Uuid", "tenant mapper input"],
  ["correlation_id: Uuid", "correlation mapper input"],
  ["let correlation_id = Uuid::new_v4();", "server-generated correlation id"],
  ["owner = ORDER_STOREFRONT_NATIVE_OWNER", "owner diagnostics"],
  ['owner_operation = "complete_storefront_checkout"', "operation diagnostics"],
  ["correlation_id = %correlation_id", "correlation diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["channel_id = ?request_context.channel_id", "channel id diagnostics"],
  ["channel_slug = ?request_context.channel_slug", "channel slug diagnostics"],
  ["locale = %request_context.locale", "locale diagnostics"],
  ["public_code = %public_code", "public code diagnostics"],
  ["public_retryable = error.retryable()", "retryability diagnostics"],
  ['code = "order.storefront_checkout_runtime_failed"', "stable internal code"],
  ["boundary = ORDER_STOREFRONT_NATIVE_BOUNDARY", "boundary diagnostics"],
  ["error = ?error", "server-side runtime cause"],
]) requireText(source, value, label);

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
  runtime_cause_logged_server_side: true,
  owner_operation_logged: true,
  correlation_logged: true,
  correlation_generated_server_side: true,
  request_context_correlation_required: false,
  idempotency_key_logged: false,
  tenant_logged: true,
  channel_logged: true,
  locale_logged: true,
  stable_code_logged: true,
  boundary_logged: true,
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
  console.error("Order storefront runtime-error diagnostics verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Order storefront checkout runtime failures retain the public envelope and use a server-generated correlation id; runtime evidence remains open",
);
