#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (source, value) => source.split(value).length - 1;

const transportPath = "crates/rustok-commerce/storefront/src/transport/mod.rs";
const safetyPath =
  "crates/rustok-commerce/storefront/src/transport/checkout_completion_command_error_safety.rs";
const orderTransportPath = "crates/rustok-order/storefront/src/transport.rs";
const orderGraphqlPath =
  "crates/rustok-order/storefront/src/transport/graphql_adapter.rs";
const orderGraphqlSafetyPath =
  "crates/rustok-order/storefront/src/transport/graphql_error_safety.rs";
const orderNativePath =
  "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs";
const cargoPath = "crates/rustok-commerce/storefront/Cargo.toml";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/storefront-checkout-command-error-safety-source.json";
const reviewPath =
  "crates/rustok-commerce/contracts/evidence/storefront-checkout-command-error-safety-source-review.json";
const docPath =
  "crates/rustok-commerce/docs/storefront-checkout-completion-command-error-safety.md";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";

const transport = read(transportPath);
const safety = read(safetyPath);
const orderTransport = read(orderTransportPath);
const orderGraphql = read(orderGraphqlPath);
const orderGraphqlSafety = read(orderGraphqlSafetyPath);
const orderNative = read(orderNativePath);
const cargo = read(cargoPath);
const evidence = JSON.parse(read(evidencePath));
const review = JSON.parse(read(reviewPath));
const doc = read(docPath);
const plan = read(planPath);

for (const marker of ["tracing.workspace = true", "uuid.workspace = true"]) {
  requireText(cargo, marker, `${cargoPath}: dependency`);
}

requireText(
  transport,
  "mod checkout_completion_command_error_safety;",
  `${transportPath}: private checkout command policy wiring`,
);
const commandStart = transport.indexOf(
  "pub async fn complete_storefront_checkout(",
);
const commandEnd = transport.indexOf("fn selected_transport_path()", commandStart);
if (commandStart < 0 || commandEnd < 0) {
  failures.push(`${transportPath}: checkout command function boundaries are missing`);
} else {
  const command = transport.slice(commandStart, commandEnd);
  for (const marker of [
    "CheckoutCompletionCommandErrorContext::new(",
    "&request,",
    "complete_checkout(request)",
    ".map_err(|error| error_context.map_error(error))",
  ]) requireText(command, marker, `${transportPath}: checkout command boundary`);
  forbidText(command, ".map_err(ApiError::from)", `${transportPath}: generic checkout mapping`);
  forbidText(command, "error.to_string()", `${transportPath}: direct checkout display mapping`);
}

if (countText(transport, ".map_err(ApiError::from)") !== 0) {
  failures.push(`${transportPath}: no Commerce owner wrapper may remain on ApiError::from`);
}
forbidText(
  transport,
  "impl From<UiTransportError> for ApiError",
  `${transportPath}: generic UiTransportError display mapper`,
);
forbidText(
  transport,
  "UiTransportError, UiTransportPath",
  `${transportPath}: stale generic mapper import`,
);

for (const [marker, label] of [
  ["pub(super) struct CheckoutCompletionCommandErrorContext", "private context"],
  ["Uuid::new_v4()", "unique correlation id"],
  [
    '"commerce-storefront-checkout:{COMMERCE_STOREFRONT_CHECKOUT_OPERATION}:{}"',
    "correlation namespace",
  ],
  ["pub(super) fn map_error(&self, error: UiTransportError)", "transport mapper"],
  ["fn is_invalid_checkout_request(error: &UiTransportError)", "validation classifier"],
  ['"cart_id must be a valid UUID"', "cart UUID compatibility"],
  [
    '"checkout idempotency key must contain 1 to 191 bytes"',
    "idempotency validation compatibility",
  ],
  ['"Checkout request is invalid"', "native validation compatibility"],
  ['"Invalid checkout request"', "validation public envelope"],
  [
    '"Checkout completion is temporarily unavailable"',
    "unavailable public envelope",
  ],
  [
    '"commerce.storefront_checkout_request_invalid"',
    "validation stable code",
  ],
  [
    '"commerce.storefront_checkout_completion_unavailable"',
    "unavailable stable code",
  ],
  ["error = ?error", "private original transport diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant shape diagnostics"],
  ["cart_id_length = self.cart_id_length", "cart shape diagnostics"],
  [
    "idempotency_key_length = self.idempotency_key_length",
    "idempotency shape diagnostics",
  ],
  ["source_module_length = self.source_module_length", "metadata module shape"],
  ["source_surface_length = self.source_surface_length", "metadata surface shape"],
  ["command_length = self.command_length", "metadata command shape"],
  ["owner_module_length = self.owner_module_length", "metadata owner shape"],
  ["create_fulfillment = self.create_fulfillment", "fulfillment flag"],
  ["failed_path = error.failed_path.as_str()", "failed path diagnostics"],
  ["fallback_attempted = error.fallback_attempted", "fallback diagnostics"],
  [
    "ApiError::Validation(INVALID_CHECKOUT_REQUEST.to_string())",
    "validation mapping",
  ],
  [
    "ApiError::ServerFn(CHECKOUT_COMPLETION_UNAVAILABLE.to_string())",
    "native mapping",
  ],
  [
    "ApiError::Graphql(CHECKOUT_COMPLETION_UNAVAILABLE.to_string())",
    "GraphQL mapping",
  ],
]) requireText(safety, marker, `${safetyPath}: ${label}`);

for (const marker of [
  "cart_id = %",
  "cart_id = ?",
  "idempotency_key = %",
  "idempotency_key = ?",
  "source_module = %",
  "source_module = ?",
  "source_surface = %",
  "source_surface = ?",
  "command = %",
  "command = ?",
  "owner_module = %",
  "owner_module = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "request = ?request",
  "ApiError::ServerFn(error.to_string())",
  "ApiError::Graphql(error.to_string())",
]) forbidText(safety, marker, `${safetyPath}: raw request or public display mapping`);

for (const marker of [
  "pub struct CompleteCheckoutRequest",
  "pub cart_id: String",
  "pub idempotency_key: String",
  "pub metadata: CheckoutCompletionCommandMetadata",
  "pub async fn complete_checkout(",
  '"complete_checkout"',
  "create_fulfillment: true",
]) requireText(orderTransport, marker, `${orderTransportPath}: owner request/operation`);
for (const marker of [
  "COMPLETE_STOREFRONT_CHECKOUT_MUTATION",
  'CheckoutCompletionTransportError::Validation("cart_id must be a valid UUID".to_string())',
  '"checkout idempotency key must contain 1 to 191 bytes"',
  "metadata.create_fulfillment",
]) requireText(orderGraphql, marker, `${orderGraphqlPath}: owner GraphQL contract`);
for (const marker of [
  '"Checkout completion is temporarily unavailable"',
  '"Checkout authentication is required"',
  '"Checkout request could not be completed"',
]) requireText(orderGraphqlSafety, marker, `${orderGraphqlSafetyPath}: owner GraphQL safety`);
for (const marker of [
  "storefront_order_complete_checkout_native(request)",
  'ServerFnError::new("Checkout request is invalid")',
  '"Checkout transport is temporarily unavailable"',
  "complete_storefront_checkout_with_product_port",
]) requireText(orderNative, marker, `${orderNativePath}: owner native contract`);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
if (
  evidence.status !==
  "commerce_storefront_checkout_command_error_safety_source_unvalidated"
) failures.push(`${evidencePath}: status mismatch`);
for (const [key, expected] of Object.entries({
  context_before_owner_call: true,
  unique_correlation_id: true,
  owner_validation_static_public_envelope: true,
  unavailable_static_public_envelope: true,
  failed_path_api_error_variant_preserved: true,
  ui_transport_display_public: false,
  generic_ui_transport_mapper_removed: true,
  raw_request_values_logged: false,
  private_transport_error_diagnostics: true,
  order_owner_transport_changed: false,
  aggregate_wrapper_changed: false,
  payment_wrapper_changed: false,
  shipping_wrapper_changed: false,
  fallback_added: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${evidencePath}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "focused_verifier_run",
  "aggregate_verifier_run",
  "broad_ecommerce_verifier_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "graphql_runtime_proven",
  "browser_runtime_proven",
  "mounted_parity_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${evidencePath}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${evidencePath}: execution must remain empty`);
}
if (
  review.status !==
  "commerce_storefront_checkout_command_error_safety_source_reviewed_unvalidated"
) failures.push(`${reviewPath}: status mismatch`);
requireText(doc, "Status: **source-ready / unvalidated**", `${docPath}: source status`);
requireText(
  doc,
  "no Commerce storefront command wrapper remains on the generic mapper",
  `${docPath}: completed Commerce wrapper scope`,
);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${planPath}: broad mapper cleanup must remain open`,
);

if (failures.length > 0) {
  console.error("Commerce storefront checkout command error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ commerce storefront checkout completion uses correlation-safe static public envelopes; no generic Commerce owner wrapper remains and runtime evidence stays open",
);
