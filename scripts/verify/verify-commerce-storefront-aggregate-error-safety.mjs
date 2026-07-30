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

const cargoPath = "crates/rustok-commerce/storefront/Cargo.toml";
const transportPath = "crates/rustok-commerce/storefront/src/transport/mod.rs";
const safetyPath =
  "crates/rustok-commerce/storefront/src/transport/aggregate_error_safety.rs";
const graphqlAdapterPath =
  "crates/rustok-commerce/storefront/src/transport/graphql_adapter.rs";
const nativeAdapterPath =
  "crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs";
const sharedAdapterPath =
  "crates/rustok-commerce/storefront/src/transport/shared_adapter.rs";
const evidencePath =
  "crates/rustok-commerce/contracts/evidence/storefront-aggregate-error-safety-source.json";

const cargo = read(cargoPath);
const transport = read(transportPath);
const safety = read(safetyPath);
const graphqlAdapter = read(graphqlAdapterPath);
const nativeAdapter = read(nativeAdapterPath);
const sharedAdapter = read(sharedAdapterPath);
const evidence = JSON.parse(read(evidencePath));

for (const [value, label] of [
  ["tracing.workspace = true", "tracing dependency"],
  ["uuid.workspace = true", "UUID dependency"],
]) requireText(cargo, value, `${cargoPath}: ${label}`);

requireText(
  transport,
  "mod aggregate_error_safety;",
  `${transportPath}: private aggregate safety module wiring`,
);
const fetchStart = transport.indexOf("pub async fn fetch_storefront_commerce(");
const nextFunction = transport.indexOf(
  "pub async fn create_storefront_payment_collection(",
  fetchStart,
);
if (fetchStart < 0 || nextFunction < 0) {
  failures.push(`${transportPath}: aggregate fetch function boundaries are missing`);
} else {
  const aggregateFetch = transport.slice(fetchStart, nextFunction);
  for (const [value, label] of [
    [
      "aggregate_error_safety::AggregateFetchErrorContext::new(&request)",
      "pre-call error context",
    ],
    [
      ".map_err(|error| error_context.map_error(error))",
      "public aggregate error mapper",
    ],
    [
      "native_server_adapter::fetch_storefront_commerce(native_request)",
      "native aggregate call",
    ],
    [
      "graphql_adapter::fetch_storefront_commerce(request)",
      "GraphQL aggregate call",
    ],
  ]) requireText(aggregateFetch, value, `${transportPath}: ${label}`);
  forbidText(
    aggregateFetch,
    ".map_err(ApiError::from)",
    `${transportPath}: aggregate fetch raw transport display mapping`,
  );
  forbidText(
    aggregateFetch,
    "error.to_string()",
    `${transportPath}: aggregate fetch direct error display mapping`,
  );
}

requireText(
  transport,
  "impl From<UiTransportError> for ApiError",
  `${transportPath}: generic command-wrapper mapper remains explicit debt`,
);
if (countText(transport, ".map_err(ApiError::from)") !== 1) {
  failures.push(
    `${transportPath}: exactly checkout completion must remain on the generic mapper`,
  );
}
for (const marker of [
  "create_storefront_payment_collection",
  "select_storefront_shipping_option",
  "complete_storefront_checkout",
]) requireText(transport, marker, `${transportPath}: preserved owner command wrapper`);

for (const [value, label] of [
  ["pub(super) struct AggregateFetchErrorContext", "private context"],
  ["Uuid::new_v4()", "unique correlation id"],
  [
    '"commerce-storefront-aggregate:{COMMERCE_STOREFRONT_AGGREGATE_OPERATION}:{}"',
    "correlation namespace",
  ],
  ["pub(super) fn map_error(&self, error: UiTransportError)", "UiTransport mapper"],
  ["fn is_invalid_cart_selection(error: &UiTransportError)", "validation classifier"],
  ['"Invalid cart selection"', "validation public envelope"],
  ['"cart_id must be a valid UUID"', "GraphQL validation compatibility"],
  [
    '"Storefront commerce data is temporarily unavailable"',
    "aggregate unavailable envelope",
  ],
  [
    '"commerce.storefront_aggregate_cart_selection_invalid"',
    "validation code",
  ],
  ['"commerce.storefront_aggregate_unavailable"', "unavailable code"],
  ["error = ?error", "private original transport diagnostics"],
  ["owner_operation = COMMERCE_STOREFRONT_AGGREGATE_OPERATION", "owner operation"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant shape diagnostics"],
  [
    "selected_cart_id_length = ?self.selected_cart_id_length",
    "cart shape diagnostics",
  ],
  ["locale_length = ?self.locale_length", "locale shape diagnostics"],
  ["failed_path = error.failed_path.as_str()", "failed path diagnostics"],
  ["fallback_attempted = error.fallback_attempted", "fallback diagnostics"],
  ["ApiError::Validation(INVALID_CART_SELECTION.to_string())", "validation mapping"],
  ["UiTransportPath::NativeServer", "native variant mapping"],
  ["UiTransportPath::Graphql", "GraphQL variant mapping"],
  [
    "ApiError::ServerFn(STOREFRONT_COMMERCE_UNAVAILABLE.to_string())",
    "native public mapping",
  ],
  [
    "ApiError::Graphql(STOREFRONT_COMMERCE_UNAVAILABLE.to_string())",
    "GraphQL public mapping",
  ],
]) requireText(safety, value, `${safetyPath}: ${label}`);

for (const value of [
  "selected_cart_id = %",
  "selected_cart_id = ?",
  "locale = %",
  "locale = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "request = ?request",
  "ApiError::ServerFn(error.to_string())",
  "ApiError::Graphql(error.to_string())",
  "ApiError::ServerFn(error.to_string",
  "ApiError::Graphql(error.to_string",
]) forbidText(safety, value, `${safetyPath}: raw public or sensitive mapping`);

requireText(
  graphqlAdapter,
  "shared_adapter::fetch_storefront_commerce_graphql(request.selected_cart_id, request.locale)",
  `${graphqlAdapterPath}: unchanged shared GraphQL aggregate delegation`,
);
requireText(
  nativeAdapter,
  "storefront_commerce_native(request.selected_cart_id, request.locale)",
  `${nativeAdapterPath}: unchanged native aggregate delegation`,
);
requireText(
  sharedAdapter,
  "pub async fn fetch_storefront_commerce_graphql(",
  `${sharedAdapterPath}: unchanged shared aggregate composition`,
);

if (evidence.schema_version !== 1) {
  failures.push(`${evidencePath}: schema_version must be 1`);
}
if (
  evidence.status !==
  "commerce_storefront_aggregate_error_safety_source_unvalidated"
) {
  failures.push(`${evidencePath}: status mismatch`);
}
for (const [key, expected] of Object.entries({
  aggregate_fetch_context_before_transport: true,
  unique_correlation_id: true,
  cart_validation_static_public_envelope: true,
  unavailable_static_public_envelope: true,
  failed_path_api_error_variant_preserved: true,
  ui_transport_display_public: false,
  raw_request_values_logged: false,
  private_transport_error_diagnostics: true,
  owner_command_wrappers_changed: false,
  graphql_adapter_changed: false,
  native_adapter_changed: false,
  shared_aggregate_composition_changed: false,
  transport_selection_changed: false,
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

if (failures.length > 0) {
  console.error("Commerce storefront aggregate error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ commerce storefront aggregate fetch uses correlation-safe static public envelopes; checkout and runtime evidence remain open",
);
