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
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};
const between = (source, start, end, label) => {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: could not isolate ${start} before ${end}`);
    return "";
  }
  return source.slice(from, to);
};

const paths = {
  facade: "crates/rustok-inventory/admin/src/transport/mod.rs",
  safety: "crates/rustok-inventory/admin/src/transport/error_safety.rs",
  native:
    "crates/rustok-inventory/admin/src/transport/native_server_adapter.rs",
  evidence:
    "crates/rustok-inventory/contracts/evidence/admin-client-transport-error-safety-source.json",
  review:
    "crates/rustok-inventory/contracts/evidence/admin-client-transport-error-safety-source-review.json",
  doc: "crates/rustok-inventory/docs/admin-client-transport-error-safety.md",
  inventoryPlan: "crates/rustok-inventory/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard: "scripts/verify/verify-inventory-admin-native-error-safety.mjs",
};

const facade = read(paths.facade);
const safety = read(paths.safety);
const native = read(paths.native);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const inventoryPlan = read(paths.inventoryPlan);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);

for (const marker of [
  "mod error_safety;",
  "pub use error_safety::InventoryTransportError;",
  "use error_safety::InventoryTransportErrorContext;",
]) {
  requireText(facade, marker, `${paths.facade}: error-safety module wiring`);
}

for (const forbidden of [
  "impl From<ServerFnError> for InventoryTransportError",
  "Self::ServerFn(value.to_string())",
  ".map_err(Into::into)",
]) {
  forbidText(facade, forbidden, `${paths.facade}: raw context-free mapping`);
}

requireCount(
  facade,
  ".map_err(|server_error| context.map_error(server_error))",
  8,
  `${paths.facade}: context-aware operation mapping`,
);

const operations = [
  ["fetch_bootstrap", "for_bootstrap", "native_server_adapter::fetch_bootstrap("],
  ["fetch_products", "for_products", "native_server_adapter::fetch_products("],
  ["fetch_product", "for_product", "native_server_adapter::fetch_product("],
  [
    "set_variant_quantity",
    "for_set_variant_quantity",
    "native_server_adapter::set_variant_quantity(",
  ],
  [
    "adjust_variant_quantity",
    "for_adjust_variant_quantity",
    "native_server_adapter::adjust_variant_quantity(",
  ],
  [
    "reserve_variant_quantity",
    "for_reserve_variant_quantity",
    "native_server_adapter::reserve_variant_quantity(",
  ],
  [
    "check_variant_availability",
    "for_check_variant_availability",
    "native_server_adapter::check_variant_availability(",
  ],
  [
    "release_reservation_quantity",
    "for_release_reservation_quantity",
    "native_server_adapter::release_reservation_quantity(",
  ],
];

for (let index = 0; index < operations.length; index += 1) {
  const [operation, constructor, call] = operations[index];
  const start = `pub async fn ${operation}(`;
  const end =
    index + 1 < operations.length
      ? `pub async fn ${operations[index + 1][0]}(`
      : "#[cfg(test)]";
  const block = between(facade, start, end, `${paths.facade}: ${operation}`);
  requireText(
    block,
    `InventoryTransportErrorContext::${constructor}`,
    `${paths.facade}: ${operation} context`,
  );
  requireText(block, call, `${paths.facade}: ${operation} native call`);
  requireText(
    block,
    ".map_err(|server_error| context.map_error(server_error))",
    `${paths.facade}: ${operation} final error mapping`,
  );
  const contextIndex = block.indexOf(`InventoryTransportErrorContext::${constructor}`);
  const callIndex = block.indexOf(call);
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${paths.facade}: ${operation} context must be created before native call`);
  }
}

for (const marker of [
  'const INVENTORY_ADMIN_CLIENT_OWNER: &str = "rustok_inventory.admin";',
  'const INVENTORY_ADMIN_CLIENT_BOUNDARY: &str = "inventory_admin_client_transport";',
  '"Inventory admin request could not be completed"',
  "pub enum InventoryTransportError",
  "ServerFn,",
  'Self::ServerFn => write!(f, "{INVENTORY_ADMIN_CLIENT_PUBLIC_MESSAGE}")',
  "pub(super) struct InventoryTransportErrorContext",
  "raw_error = ?error",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "tenant_id_present = self.tenant_id_length.is_some()",
  "tenant_id_length = ?self.tenant_id_length",
  "subject_id_present = self.subject_id_length.is_some()",
  "subject_id_length = ?self.subject_id_length",
  "locale_present = self.locale_length.is_some()",
  "search_present = self.search_length.is_some()",
  "status_present = self.status_length.is_some()",
  "numeric_input_present = self.numeric_input_present",
  'code = "inventory.admin_client_transport_failed"',
  "boundary = INVENTORY_ADMIN_CLIENT_BOUNDARY",
  "InventoryTransportError::ServerFn",
]) {
  requireText(safety, marker, `${paths.safety}: safe public mapping`);
}

for (const forbidden of [
  "ServerFn(String)",
  "tenant_id = %",
  "subject_id = %",
  "locale = %",
  "search = %",
  "status = %",
  "quantity =",
  "adjustment =",
  "requested_quantity =",
  "error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: raw request/public text`);
}

for (const operation of operations.map(([name]) => name)) {
  requireText(
    safety,
    `"${operation}"`,
    `${paths.safety}: stable operation ${operation}`,
  );
}

for (const endpoint of [
  'endpoint = "inventory/bootstrap"',
  'endpoint = "inventory/products"',
  'endpoint = "inventory/product"',
  'endpoint = "inventory/variant/set-quantity"',
  'endpoint = "inventory/variant/adjust-quantity"',
  'endpoint = "inventory/variant/reserve-quantity"',
  'endpoint = "inventory/variant/check-availability"',
  'endpoint = "inventory/variant/release-reservation"',
]) {
  requireText(native, endpoint, `${paths.native}: preserved mounted endpoint`);
}
requireText(
  nativeGuard,
  "Inventory admin native endpoints use static public envelopes",
  `${paths.nativeGuard}: prior native policy remains registered`,
);

if (evidence.status !== "inventory_admin_client_transport_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "inventory_admin_client_transport_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}
if (review.findings?.public_error_variant_has_no_string_payload !== true) {
  failures.push(`${paths.review}: unit public error variant review is required`);
}
for (const [key, expected] of Object.entries({
  server_functions_changed: false,
  request_normalization_changed: false,
  dto_changed: false,
  operation_count: 8,
  context_created_before_native_call: true,
  raw_server_fn_string_public: false,
  public_error_payload_constructible: false,
  static_public_message: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  tenant_values_logged: false,
  subject_values_logged: false,
  filter_values_logged: false,
  numeric_values_logged: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "hydrate_compile_proven",
  "ssr_compile_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "eight", `${paths.doc}: operation scope`);
requireText(
  doc,
  "Inventory admin request could not be completed",
  `${paths.doc}: static public message`,
);
requireText(doc, "unit variant", `${paths.doc}: fail-closed construction`);
requireText(
  inventoryPlan,
  "Admin client transport error safety: `source_ready_unvalidated`",
  `${paths.inventoryPlan}: local status`,
);
requireText(
  inventoryPlan,
  "verify-inventory-admin-client-transport-error-safety.mjs",
  `${paths.inventoryPlan}: local verifier`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Inventory Admin client transport error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Inventory Admin client transport errors use correlation-safe static public text across eight native operations; execution evidence remains open",
);
