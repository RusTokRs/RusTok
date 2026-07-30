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
  "crates/rustok-commerce/storefront/src/transport/payment_collection_command_error_safety.rs";
const paymentTransportPath = "crates/rustok-payment/storefront/src/transport.rs";
const paymentGraphqlPath =
  "crates/rustok-payment/storefront/src/transport/graphql_adapter.rs";
const paymentNativePath =
  "crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs";
const cargoPath = "crates/rustok-commerce/storefront/Cargo.toml";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/storefront-payment-command-error-safety-source.json";
const reviewPath =
  "crates/rustok-commerce/contracts/evidence/storefront-payment-command-error-safety-source-review.json";
const docPath =
  "crates/rustok-commerce/docs/storefront-payment-collection-command-error-safety.md";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";

const transport = read(transportPath);
const safety = read(safetyPath);
const paymentTransport = read(paymentTransportPath);
const paymentGraphql = read(paymentGraphqlPath);
const paymentNative = read(paymentNativePath);
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
  "mod payment_collection_command_error_safety;",
  `${transportPath}: private payment command policy wiring`,
);
const commandStart = transport.indexOf(
  "pub async fn create_storefront_payment_collection(",
);
const commandEnd = transport.indexOf(
  "pub async fn select_storefront_shipping_option(",
  commandStart,
);
if (commandStart < 0 || commandEnd < 0) {
  failures.push(`${transportPath}: payment command function boundaries are missing`);
} else {
  const command = transport.slice(commandStart, commandEnd);
  for (const marker of [
    "PaymentCollectionCommandErrorContext::new(",
    "&request,",
    "create_payment_collection(request)",
    ".map_err(|error| error_context.map_error(error))",
  ]) requireText(command, marker, `${transportPath}: payment command boundary`);
  forbidText(command, ".map_err(ApiError::from)", `${transportPath}: generic payment mapping`);
  forbidText(command, "error.to_string()", `${transportPath}: direct payment display mapping`);
}

if (countText(transport, ".map_err(ApiError::from)") !== 1) {
  failures.push(
    `${transportPath}: exactly checkout completion must remain on the generic mapper`,
  );
}
for (const marker of [
  "select_shipping_option(request.owner_request)",
  "complete_checkout(request).await.map_err(ApiError::from)",
]) requireText(transport, marker, `${transportPath}: preserved later command boundary`);

for (const [marker, label] of [
  ["pub(super) struct PaymentCollectionCommandErrorContext", "private context"],
  ["Uuid::new_v4()", "unique correlation id"],
  [
    '"commerce-storefront-payment:{COMMERCE_STOREFRONT_PAYMENT_OPERATION}:{}"',
    "correlation namespace",
  ],
  ["pub(super) fn map_error(&self, error: UiTransportError)", "transport mapper"],
  ["fn is_invalid_cart_selection(error: &UiTransportError)", "validation classifier"],
  ['"cart_id must be a valid UUID"', "owner validation compatibility"],
  ['"Invalid cart selection"', "validation public envelope"],
  [
    '"Storefront payment collection is temporarily unavailable"',
    "unavailable public envelope",
  ],
  [
    '"commerce.storefront_payment_collection_cart_id_invalid"',
    "validation code",
  ],
  [
    '"commerce.storefront_payment_collection_unavailable"',
    "unavailable code",
  ],
  ["error = ?error", "private original transport diagnostics"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant shape diagnostics"],
  ["cart_id_length = self.cart_id_length", "cart shape diagnostics"],
  ["source_module_length = self.source_module_length", "metadata shape diagnostics"],
  ["source_surface_length = self.source_surface_length", "surface shape diagnostics"],
  ["command_length = self.command_length", "command shape diagnostics"],
  ["owner_module_length = self.owner_module_length", "owner shape diagnostics"],
  ["failed_path = error.failed_path.as_str()", "failed path diagnostics"],
  ["fallback_attempted = error.fallback_attempted", "fallback diagnostics"],
  ["ApiError::Validation(INVALID_CART_SELECTION.to_string())", "validation mapping"],
  ["ApiError::ServerFn(STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE.to_string())", "native mapping"],
  ["ApiError::Graphql(STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE.to_string())", "GraphQL mapping"],
]) requireText(safety, marker, `${safetyPath}: ${label}`);

for (const marker of [
  "cart_id = %",
  "cart_id = ?",
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
  "pub struct PaymentCollectionCreateRequest",
  "pub cart_id: String",
  "pub metadata: PaymentCollectionCommandMetadata",
  "pub async fn create_payment_collection(",
  '"create_storefront_payment_collection"',
]) requireText(paymentTransport, marker, `${paymentTransportPath}: owner request/operation`);
for (const marker of [
  "CREATE_STOREFRONT_PAYMENT_COLLECTION_MUTATION",
  "let cart_id = parse_cart_id(&request.cart_id)?;",
  'PaymentTransportError::Validation(format!("{field} must be a valid UUID"))',
]) requireText(paymentGraphql, marker, `${paymentGraphqlPath}: owner GraphQL contract`);
for (const marker of [
  "storefront_payment_create_collection_native(request)",
  'ServerFnError::new("cart_id must be a valid UUID")',
  '"Storefront payment collection is temporarily unavailable"',
]) requireText(paymentNative, marker, `${paymentNativePath}: owner native contract`);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
if (
  evidence.status !==
  "commerce_storefront_payment_command_error_safety_source_unvalidated"
) failures.push(`${evidencePath}: status mismatch`);
for (const [key, expected] of Object.entries({
  context_before_owner_call: true,
  unique_correlation_id: true,
  cart_validation_static_public_envelope: true,
  unavailable_static_public_envelope: true,
  failed_path_api_error_variant_preserved: true,
  ui_transport_display_public: false,
  raw_request_values_logged: false,
  private_transport_error_diagnostics: true,
  payment_owner_transport_changed: false,
  shipping_wrapper_changed: false,
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
if (review.status !== "commerce_storefront_payment_command_error_safety_source_reviewed_unvalidated") {
  failures.push(`${reviewPath}: status mismatch`);
}
requireText(doc, "Status: **source-ready / unvalidated**", `${docPath}: source status`);
requireText(doc, "shipping-selection and checkout-completion wrappers remain open", `${docPath}: historical remaining wrapper scope`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${planPath}: broad mapper cleanup must remain open`,
);

if (failures.length > 0) {
  console.error("Commerce storefront payment command error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ commerce storefront payment collection command uses correlation-safe static public envelopes; checkout and runtime evidence remain open",
);
