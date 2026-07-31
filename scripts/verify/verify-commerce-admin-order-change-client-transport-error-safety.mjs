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

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const paths = {
  routing: "crates/rustok-commerce/admin/src/transport/mod.rs",
  facade: "crates/rustok-commerce/admin/src/transport/order_change.rs",
  safety:
    "crates/rustok-commerce/admin/src/transport/order_change_client_error_safety.rs",
  native: "crates/rustok-commerce/admin/src/transport/native_server_adapter.rs",
  nativeSsr:
    "crates/rustok-commerce/admin/src/transport/native_server_adapter_ssr.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-order-change-client-transport-error-safety-source.json",
  review:
    "crates/rustok-commerce/contracts/evidence/admin-order-change-client-transport-error-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-order-change-client-transport-error-safety.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard:
    "scripts/verify/verify-commerce-admin-order-change-native-error-safety.mjs",
};

const routing = read(paths.routing);
const facade = read(paths.facade);
const safety = read(paths.safety);
const native = read(paths.native);
const nativeSsr = read(paths.nativeSsr);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);

requireText(
  routing,
  "mod order_change_client_error_safety;",
  `${paths.routing}: client error policy wiring`,
);
requireText(
  routing,
  "pub use order_change::{apply_order_change, cancel_order_change, fetch_order_changes};",
  `${paths.routing}: order-change exports`,
);

for (const marker of [
  "order_change_client_error_safety::OrderChangeClientErrorContext",
  "Result<CommerceOrderChangeList, ApiError>",
  "Result<CommerceOrderChange, ApiError>",
  "if use_graphql_transport()",
]) {
  requireText(facade, marker, `${paths.facade}: preserved facade contract`);
}
requireCount(
  facade,
  "map_graphql_error(",
  3,
  `${paths.facade}: preserved GraphQL error mappings`,
);
requireCount(
  facade,
  ".map_err(|order_change_error| context.map_error(order_change_error))",
  3,
  `${paths.facade}: final native client mappings`,
);

for (const [operation, constructor, nativeCall] of [
  [
    "fetch_order_changes",
    "OrderChangeClientErrorContext::for_fetch",
    "native_server_adapter::fetch_order_changes(",
  ],
  [
    "apply_order_change",
    "OrderChangeClientErrorContext::for_apply",
    "native_server_adapter::apply_order_change(",
  ],
  [
    "cancel_order_change",
    "OrderChangeClientErrorContext::for_cancel",
    "native_server_adapter::cancel_order_change(",
  ],
]) {
  const body = functionBody(facade, operation);
  requireText(body, constructor, `${paths.facade}: ${operation} context`);
  requireText(body, nativeCall, `${paths.facade}: ${operation} native call`);
  requireText(
    body,
    ".map_err(|order_change_error| context.map_error(order_change_error))",
    `${paths.facade}: ${operation} final mapping`,
  );
  const elseIndex = body.indexOf("} else {");
  const contextIndex = body.indexOf(constructor);
  const nativeIndex = body.indexOf(nativeCall);
  if (
    elseIndex < 0 ||
    contextIndex < 0 ||
    nativeIndex < 0 ||
    contextIndex < elseIndex ||
    contextIndex > nativeIndex
  ) {
    failures.push(
      `${paths.facade}: ${operation} context must be inside the native branch before the adapter call`,
    );
  }
}

for (const marker of [
  'const COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_OWNER: &str =',
  '"rustok_commerce.admin_order_change_transport"',
  'const COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_BOUNDARY: &str =',
  '"commerce_admin_order_change_client_transport"',
  '"Commerce admin order-change request could not be completed"',
  "struct OrderChangeClientErrorFacts",
  "error_variant: &'static str",
  "message_present: bool",
  "message_length: usize",
  "pub(super) struct OrderChangeClientErrorContext",
  'operation: "fetch_order_changes"',
  'Self::for_write(\n            "apply_order_change"',
  'Self::for_write(\n            "cancel_order_change"',
  "let error_facts = order_change_client_error_facts(&error);",
  "error_variant = error_facts.error_variant",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "token_present = self.token_present",
  "tenant_slug_present = self.tenant_slug_length.is_some()",
  "tenant_id_present = self.tenant_id_length > 0",
  "order_id_present = self.order_id_length.is_some()",
  "order_change_id_present = self.order_change_id_length.is_some()",
  "status_present = self.status_length.is_some()",
  "payload_present = self.payload_present",
  'code = "commerce.admin_order_change_client_transport_failed"',
  "boundary = COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_BOUNDARY",
  "ApiError::ServerFn(COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_PUBLIC_MESSAGE.to_string())",
  "fn order_change_client_error_facts(error: &ApiError)",
  'ApiError::Graphql(message) => ("graphql", message)',
  'ApiError::ServerFn(message) => ("server_fn", message)',
  "message_present: !message.trim().is_empty()",
  "message_length: message.chars().count()",
]) {
  requireText(safety, marker, `${paths.safety}: safe final mapping`);
}

const mapperBody = functionBody(safety, "map_error");
for (const forbidden of [
  "raw_error = ?error",
  "raw_error = %error",
  "error = ?error",
  "error = %error",
  "message = %error",
  "message = ?error",
  "error.to_string()",
]) {
  forbidText(mapperBody, forbidden, `${paths.safety}: complete error diagnostics`);
}

for (const forbidden of [
  "token = %",
  "token = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "tenant_id = %",
  "tenant_id = ?",
  "order_id = %",
  "order_id = ?",
  "order_change_id = %",
  "order_change_id = ?",
  "status = %",
  "status = ?",
  "draft = ?",
  "payload = ?",
]) {
  forbidText(safety, forbidden, `${paths.safety}: raw request value`);
}

for (const source of [native, nativeSsr]) {
  for (const [operation, nativeCall] of [
    [
      "fetch_order_changes",
      "commerce_admin_order_changes_native(tenant_id, order_id, status)",
    ],
    [
      "apply_order_change",
      "commerce_admin_apply_order_change_native(tenant_id, id, draft)",
    ],
    [
      "cancel_order_change",
      "commerce_admin_cancel_order_change_native(tenant_id, id, draft)",
    ],
  ]) {
    const body = functionBody(source, operation);
    requireText(body, nativeCall, `native adapter: preserved ${operation} call`);
    requireText(body, ".map_err(Into::into)", `native adapter: preserved ${operation} mapping`);
  }
}

for (const endpoint of [
  'endpoint = "commerce/admin/order-changes"',
  'endpoint = "commerce/admin/apply-order-change"',
  'endpoint = "commerce/admin/cancel-order-change"',
]) {
  requireText(nativeSsr, endpoint, `${paths.nativeSsr}: preserved endpoint`);
}
requireText(
  nativeGuard,
  "Commerce admin order-change native diagnostics use correlation-safe shape only",
  `${paths.nativeGuard}: prior server-side guard remains registered`,
);

if (
  evidence.status !==
  "commerce_admin_order_change_client_transport_error_safety_source_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "commerce_admin_order_change_client_transport_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  native_adapters_changed: false,
  server_functions_changed: false,
  graphql_transport_changed: false,
  promotion_transport_changed: false,
  request_response_dto_changed: false,
  api_error_contract_preserved: true,
  operation_count: 3,
  context_created_before_native_call: true,
  raw_native_error_public: false,
  static_public_message: true,
  original_error_logged_privately: false,
  original_error_shape_logged: true,
  error_variant_logged: true,
  error_message_length_only: true,
  raw_native_error_logged: false,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  token_values_logged: false,
  tenant_values_logged: false,
  order_values_logged: false,
  status_values_logged: false,
  action_payload_values_logged: false,
  broad_ecommerce_cleanup_closed: false,
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
requireText(
  doc,
  "Commerce admin order-change request could not be completed",
  `${paths.doc}: static public message`,
);
requireText(
  doc,
  "complete `ApiError` is not logged",
  `${paths.doc}: private diagnostic policy`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce Admin order-change client transport error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce Admin order-change client diagnostics retain only ApiError variant and message shape; execution evidence remains open",
);
