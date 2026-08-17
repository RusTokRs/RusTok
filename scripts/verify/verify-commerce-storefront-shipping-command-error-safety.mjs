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
  "crates/rustok-commerce/storefront/src/transport/shipping_option_command_error_safety.rs";
const fulfillmentTransportPath = "crates/rustok-fulfillment/storefront/src/transport.rs";
const fulfillmentGraphqlPath =
  "crates/rustok-fulfillment/storefront/src/transport/graphql_adapter.rs";
const fulfillmentNativePath =
  "crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs";
const cargoPath = "crates/rustok-commerce/storefront/Cargo.toml";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/storefront-shipping-command-error-safety-source.json";
const reviewPath =
  "crates/rustok-commerce/contracts/evidence/storefront-shipping-command-error-safety-source-review.json";
const docPath =
  "crates/rustok-commerce/docs/storefront-shipping-option-command-error-safety.md";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";

const transport = read(transportPath);
const safety = read(safetyPath);
const fulfillmentTransport = read(fulfillmentTransportPath);
const fulfillmentGraphql = read(fulfillmentGraphqlPath);
const fulfillmentNative = read(fulfillmentNativePath);
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
  "mod shipping_option_command_error_safety;",
  `${transportPath}: private shipping command policy wiring`,
);
const commandStart = transport.indexOf(
  "pub async fn select_storefront_shipping_option(",
);
const commandEnd = transport.indexOf(
  "pub async fn complete_storefront_checkout(",
  commandStart,
);
if (commandStart < 0 || commandEnd < 0) {
  failures.push(`${transportPath}: shipping command function boundaries are missing`);
} else {
  const command = transport.slice(commandStart, commandEnd);
  for (const marker of [
    "ShippingOptionCommandErrorContext::new(&request)",
    "select_shipping_option(request.owner_request)",
    ".map_err(|error| error_context.map_error(error))",
  ]) requireText(command, marker, `${transportPath}: shipping command boundary`);
  forbidText(command, ".map_err(ApiError::from)", `${transportPath}: generic shipping mapping`);
  forbidText(command, "error.to_string()", `${transportPath}: direct shipping display mapping`);
}

if (countText(transport, ".map_err(ApiError::from)") !== 0) {
  failures.push(
    `${transportPath}: no Commerce owner wrapper may remain on the generic mapper`,
  );
}
forbidText(
  transport,
  "impl From<UiTransportError> for ApiError",
  `${transportPath}: generic UiTransportError display mapper`,
);
requireText(
  transport,
  "complete_checkout(request)",
  `${transportPath}: preserved checkout command boundary`,
);

for (const [marker, label] of [
  ["pub(super) struct ShippingOptionCommandErrorContext", "private context"],
  ["Uuid::new_v4()", "unique correlation id"],
  [
    '"commerce-storefront-shipping:{COMMERCE_STOREFRONT_SHIPPING_OPERATION}:{}"',
    "correlation namespace",
  ],
  ["pub(super) fn map_error(&self, error: UiTransportError)", "transport mapper"],
  ["fn is_invalid_shipping_selection(error: &UiTransportError)", "validation classifier"],
  ["fn is_shipping_selection_validation_message(message: &str)", "validation policy"],
  ['"cart_id must be a valid UUID"', "cart UUID compatibility"],
  [
    '"selected_shipping_option_id must be a valid UUID"',
    "shipping option UUID compatibility",
  ],
  ['message.starts_with("delivery group `")', "missing delivery group compatibility"],
  [
    'message.contains(" is not available for shipping profile ")',
    "unavailable option compatibility",
  ],
  ['"Invalid shipping selection"', "validation public envelope"],
  ['"Shipping selection is temporarily unavailable"', "unavailable public envelope"],
  [
    '"commerce.storefront_shipping_selection_invalid"',
    "validation stable code",
  ],
  [
    '"commerce.storefront_shipping_selection_unavailable"',
    "unavailable stable code",
  ],
  ["error = ?error", "private original transport diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant shape diagnostics"],
  ["cart_id_length = self.cart_id_length", "cart shape diagnostics"],
  ["delivery_group_count = self.delivery_group_count", "group count diagnostics"],
  [
    "available_shipping_option_count = self.available_shipping_option_count",
    "available option count diagnostics",
  ],
  [
    "shipping_profile_slug_length = self.shipping_profile_slug_length",
    "shipping profile shape diagnostics",
  ],
  ["seller_id_length = ?self.seller_id_length", "seller shape diagnostics"],
  [
    "shipping_option_id_length = ?self.shipping_option_id_length",
    "shipping option shape diagnostics",
  ],
  ["failed_path = error.failed_path.as_str()", "failed path diagnostics"],
  ["fallback_attempted = error.fallback_attempted", "fallback diagnostics"],
  [
    "ApiError::Validation(INVALID_SHIPPING_SELECTION.to_string())",
    "validation mapping",
  ],
  [
    "ApiError::ServerFn(SHIPPING_SELECTION_UNAVAILABLE.to_string())",
    "native mapping",
  ],
  [
    "ApiError::Graphql(SHIPPING_SELECTION_UNAVAILABLE.to_string())",
    "GraphQL mapping",
  ],
]) requireText(safety, marker, `${safetyPath}: ${label}`);

for (const marker of [
  "cart_id = %",
  "cart_id = ?",
  "shipping_profile_slug = %",
  "shipping_profile_slug = ?",
  "seller_id = %",
  "seller_id = ?",
  "shipping_option_id = %",
  "shipping_option_id = ?",
  "available_shipping_option_ids =",
  "delivery_groups =",
  "tenant_slug = %",
  "tenant_slug = ?",
  "request = ?request",
  "ApiError::ServerFn(error.to_string())",
  "ApiError::Graphql(error.to_string())",
]) forbidText(safety, marker, `${safetyPath}: raw request or public display mapping`);

for (const marker of [
  "pub struct SelectShippingOptionRequest",
  "pub cart_id: String",
  "pub delivery_groups: Vec<ShippingSelectionDeliveryGroup>",
  "pub shipping_profile_slug: String",
  "pub seller_id: Option<String>",
  "pub shipping_option_id: Option<String>",
  "pub async fn select_shipping_option(",
  "pub fn build_shipping_selection_updates(",
]) requireText(fulfillmentTransport, marker, `${fulfillmentTransportPath}: owner request/plan`);
for (const marker of [
  "SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION",
  'parse_required_uuid(&request.cart_id, "cart_id")?',
  '"selected_shipping_option_id"',
  'ShippingSelectionTransportError::Validation(format!("{field_name} must be a valid UUID"))',
]) requireText(fulfillmentGraphql, marker, `${fulfillmentGraphqlPath}: owner GraphQL contract`);
for (const marker of [
  "storefront_fulfillment_select_shipping_option_native(request)",
  'ServerFnError::new("cart_id must be a valid UUID")',
  '"selected_shipping_option_id"',
  'ServerFnError::new("Shipping selection is temporarily unavailable")',
]) requireText(fulfillmentNative, marker, `${fulfillmentNativePath}: owner native contract`);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
if (
  evidence.status !==
  "commerce_storefront_shipping_command_error_safety_source_unvalidated"
) failures.push(`${evidencePath}: status mismatch`);
for (const [key, expected] of Object.entries({
  context_before_owner_call: true,
  unique_correlation_id: true,
  owner_validation_static_public_envelope: true,
  unavailable_static_public_envelope: true,
  failed_path_api_error_variant_preserved: true,
  ui_transport_display_public: false,
  raw_request_values_logged: false,
  private_transport_error_diagnostics: true,
  fulfillment_owner_transport_changed: false,
  payment_wrapper_changed: false,
  checkout_wrapper_changed: false,
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
  "commerce_storefront_shipping_command_error_safety_source_reviewed_unvalidated"
) failures.push(`${reviewPath}: status mismatch`);
requireText(doc, "Status: **source-ready / unvalidated**", `${docPath}: source status`);
requireText(doc, "checkout-completion wrapper remains open", `${docPath}: historical remaining wrapper scope`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${planPath}: broad mapper cleanup must remain open`,
);

if (failures.length > 0) {
  console.error("Commerce storefront shipping command error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ commerce storefront shipping option command uses correlation-safe static public envelopes; no generic Commerce owner wrapper remains and runtime evidence stays open",
);
