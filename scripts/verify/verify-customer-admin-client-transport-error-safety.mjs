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
  facade: "crates/rustok-customer/admin/src/transport/mod.rs",
  safety: "crates/rustok-customer/admin/src/transport/error_safety.rs",
  native: "crates/rustok-customer/admin/src/transport/native_server_adapter.rs",
  evidence:
    "crates/rustok-customer/contracts/evidence/admin-client-transport-error-safety-source.json",
  review:
    "crates/rustok-customer/contracts/evidence/admin-client-transport-error-safety-source-review.json",
  doc: "crates/rustok-customer/docs/admin-client-transport-error-safety.md",
  customerPlan: "crates/rustok-customer/docs/implementation-plan.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard: "scripts/verify/verify-customer-admin-native-error-safety.mjs",
};

const facade = read(paths.facade);
const safety = read(paths.safety);
const native = read(paths.native);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const customerPlan = read(paths.customerPlan);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);

for (const marker of [
  "mod error_safety;",
  "mod native_server_adapter;",
  "pub use error_safety::ApiError;",
  "use error_safety::CustomerAdminTransportErrorContext;",
]) {
  requireText(facade, marker, `${paths.facade}: client safety wiring`);
}

for (const forbidden of [
  "pub use native_server_adapter::ApiError;",
  ".map_err(Into::into)",
  "ApiError::ServerFn(value.to_string())",
]) {
  forbidText(facade, forbidden, `${paths.facade}: private native error escape`);
}

requireCount(
  facade,
  ".map_err(|server_error| context.map_error(server_error))",
  5,
  `${paths.facade}: context-aware final mappings`,
);

const operations = [
  ["fetch_bootstrap", "for_bootstrap", "native::fetch_bootstrap("],
  ["fetch_customers", "for_customers", "native::fetch_customers("],
  [
    "fetch_customer_detail",
    "for_customer_detail",
    "native::fetch_customer_detail(",
  ],
  ["create_customer", "for_create_customer", "native::create_customer("],
  ["update_customer", "for_update_customer", "native::update_customer("],
];

for (let index = 0; index < operations.length; index += 1) {
  const [operation, constructor, call] = operations[index];
  const start = `pub async fn ${operation}(`;
  const end =
    index + 1 < operations.length
      ? `pub async fn ${operations[index + 1][0]}(`
      : null;
  const from = facade.indexOf(start);
  const block =
    end === null
      ? from < 0
        ? ""
        : facade.slice(from)
      : between(facade, start, end, `${paths.facade}: ${operation}`);
  if (from < 0) failures.push(`${paths.facade}: missing ${start}`);
  requireText(
    block,
    `CustomerAdminTransportErrorContext::${constructor}`,
    `${paths.facade}: ${operation} context`,
  );
  requireText(block, call, `${paths.facade}: ${operation} native call`);
  requireText(
    block,
    ".map_err(|server_error| context.map_error(server_error))",
    `${paths.facade}: ${operation} mapping`,
  );
  const contextIndex = block.indexOf(
    `CustomerAdminTransportErrorContext::${constructor}`,
  );
  const callIndex = block.indexOf(call);
  if (contextIndex < 0 || callIndex < 0 || contextIndex > callIndex) {
    failures.push(`${paths.facade}: ${operation} context must precede native call`);
  }
}

for (const marker of [
  'const CUSTOMER_ADMIN_CLIENT_OWNER: &str = "rustok_customer.admin";',
  'const CUSTOMER_ADMIN_CLIENT_BOUNDARY: &str = "customer_admin_client_transport";',
  '"Customer admin request could not be completed"',
  "pub enum ApiError {",
  "ServerFn,",
  "Self::ServerFn => f.write_str(CUSTOMER_ADMIN_CLIENT_PUBLIC_MESSAGE)",
  "pub(super) struct CustomerAdminTransportErrorContext",
  "raw_error = ?error",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "subject_id_present = self.subject_id_length.is_some()",
  "search_present = self.search_length.is_some()",
  "pagination_present = self.pagination_present",
  "payload_present = self.payload_present",
  'code = "customer.admin_client_transport_failed"',
  "boundary = CUSTOMER_ADMIN_CLIENT_BOUNDARY",
  "ApiError::ServerFn",
]) {
  requireText(safety, marker, `${paths.safety}: fail-closed client mapping`);
}

for (const forbidden of [
  "ServerFn(String)",
  "ApiError::ServerFn(",
  "customer_id = %",
  "search = %",
  "email = %",
  "payload = ?",
  "page =",
  "per_page =",
  "error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: public payload or raw request value`);
}

for (const operation of operations.map(([name]) => name)) {
  requireText(safety, `"${operation}"`, `${paths.safety}: stable operation ${operation}`);
}

for (const marker of [
  "pub enum ApiError",
  "ServerFn(String)",
  "Self::ServerFn(value.to_string())",
  ".map_err(Into::into)",
  'endpoint = "customer/bootstrap"',
  'endpoint = "customer/list"',
  'endpoint = "customer/detail"',
  'endpoint = "customer/create"',
  'endpoint = "customer/update"',
]) {
  requireText(native, marker, `${paths.native}: private compatibility source preserved`);
}

requireText(
  nativeGuard,
  "customer admin native transport uses static public envelopes",
  `${paths.nativeGuard}: prior server-side policy remains registered`,
);

if (evidence.status !== "customer_admin_client_transport_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "customer_admin_client_transport_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  native_server_adapter_changed: false,
  server_functions_changed: false,
  request_response_dto_changed: false,
  operation_count: 5,
  context_created_before_native_call: true,
  private_native_error_reexported: false,
  public_error_string_payload: false,
  static_public_message: true,
  original_error_logged_privately: true,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  customer_id_values_logged: false,
  search_values_logged: false,
  payload_values_logged: false,
  pagination_values_logged: false,
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
requireText(doc, "Customer admin request could not be completed", `${paths.doc}: public message`);
requireText(
  customerPlan,
  "Admin client transport error safety: `source_ready_unvalidated`",
  `${paths.customerPlan}: local status`,
);
requireText(
  customerPlan,
  "verify-customer-admin-client-transport-error-safety.mjs",
  `${paths.customerPlan}: local verifier`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Customer Admin client transport error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Customer Admin client transport errors use a payload-free static public envelope across five native operations; execution evidence remains open",
);
