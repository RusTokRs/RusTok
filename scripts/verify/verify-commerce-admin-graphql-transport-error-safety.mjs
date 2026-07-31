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

const paths = {
  safety: "crates/rustok-commerce/admin/src/transport/graphql_error_safety.rs",
  orderChange: "crates/rustok-commerce/admin/src/transport/order_change.rs",
  shippingProfile: "crates/rustok-commerce/admin/src/transport/shipping_profile.rs",
  promotion: "crates/rustok-commerce/admin/src/transport/promotion.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-graphql-transport-error-safety-source.json",
  review:
    "crates/rustok-commerce/contracts/evidence/admin-graphql-transport-error-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-graphql-transport-error-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const safety = read(paths.safety);
const orderChange = read(paths.orderChange);
const shippingProfile = read(paths.shippingProfile);
const promotion = read(paths.promotion);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const plan = read(paths.plan);

for (const marker of [
  'const COMMERCE_ADMIN_GRAPHQL_CONSUMER: &str = "rustok_commerce.admin_graphql_transport"',
  'const COMMERCE_ADMIN_GRAPHQL_BOUNDARY: &str = "commerce_admin_graphql_transport"',
  "struct CommerceAdminGraphqlErrorFacts",
  "error_payload_present: bool",
  "error_payload_length: usize",
  "parse_succeeded: bool",
  "detail_present: bool",
  "detail_length: usize",
  "let ApiError::Graphql(raw_error) = error else",
  "return error;",
  "let parsed = GraphqlHttpError::from_str(raw_error.as_str());",
  "let error_facts = commerce_admin_graphql_error_facts(raw_error.as_str(), &parsed);",
  "tenant_id_present = tenant_id_length.is_some()",
  "tenant_id_length",
  "tenant_id_uuid_valid = tenant_uuid.is_some()",
  "tenant_id_uuid_non_nil = tenant_uuid.as_ref().is_some_and(|value| !value.is_nil())",
  "error_payload_present = error_facts.error_payload_present",
  "error_payload_length = error_facts.error_payload_length",
  "parse_succeeded = error_facts.parse_succeeded",
  "error_detail_present = error_facts.detail_present",
  "error_detail_length = error_facts.detail_length",
  "fn commerce_admin_graphql_error_facts(",
  "error_payload_present: !raw_error.trim().is_empty()",
  "error_payload_length: raw_error.chars().count()",
  "parse_succeeded: parsed.is_ok()",
  "detail_length: detail.map_or(0, |value| value.chars().count())",
]) {
  requireText(safety, marker, `${paths.safety}: safe diagnostic shape`);
}

for (const forbidden of [
  "error = %raw_error",
  "error = ?raw_error",
  "raw_error = %raw_error",
  "raw_error = ?raw_error",
  "parsed_error = ?parsed",
  "parsed_error = %parsed",
  "tenant_id = %",
  "tenant_id = ?",
  "tenant_id,\n            tenant_slug_present",
  "error.to_string()",
  "raw_error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: complete payload or identity logging`);
}

for (const [code, message, kind] of [
  [
    "commerce.admin_graphql_authentication_required",
    "Commerce admin authentication is required",
    "unauthorized",
  ],
  [
    "commerce.admin_graphql_network_unavailable",
    "Commerce admin service is temporarily unavailable",
    "network",
  ],
  [
    "commerce.admin_graphql_http_unavailable",
    "Commerce admin service is temporarily unavailable",
    "http",
  ],
  [
    "commerce.admin_graphql_request_rejected",
    "Commerce admin request could not be completed",
    "graphql",
  ],
  [
    "commerce.admin_graphql_unknown_failure",
    "Commerce admin request could not be completed",
    "unknown",
  ],
]) {
  requireText(safety, code, `${paths.safety}: preserved public code`);
  requireText(safety, message, `${paths.safety}: preserved public message`);
  requireText(safety, `"${kind}"`, `${paths.safety}: preserved error kind`);
}
requireText(
  safety,
  "ApiError::Graphql(public_message.to_string())",
  `${paths.safety}: public ApiError envelope`,
);
requireText(safety, "tracing::error!(", `${paths.safety}: severe diagnostics`);
requireText(safety, "tracing::warn!(", `${paths.safety}: rejection diagnostics`);

requireCount(
  orderChange,
  "map_graphql_error(",
  3,
  `${paths.orderChange}: order-change GraphQL mappings`,
);
requireCount(
  shippingProfile,
  "map_graphql_error(",
  7,
  `${paths.shippingProfile}: shipping-profile GraphQL mappings`,
);
for (const operation of [
  "fetch_order_changes",
  "apply_order_change",
  "cancel_order_change",
]) {
  requireText(orderChange, `pub async fn ${operation}(`, `${paths.orderChange}: ${operation}`);
}
for (const operation of [
  "fetch_bootstrap",
  "fetch_shipping_profiles",
  "fetch_shipping_profile",
  "create_shipping_profile",
  "update_shipping_profile",
  "deactivate_shipping_profile",
  "reactivate_shipping_profile",
]) {
  requireText(
    shippingProfile,
    `pub async fn ${operation}(`,
    `${paths.shippingProfile}: ${operation}`,
  );
}
forbidText(
  promotion,
  "map_graphql_error(",
  `${paths.promotion}: promotion remains on its native client policy`,
);

if (evidence.status !== "commerce_admin_graphql_transport_error_safety_source_unvalidated") {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  shared_mapper_changed: true,
  order_change_facade_changed: false,
  shipping_profile_facade_changed: false,
  graphql_adapter_changed: false,
  promotion_transport_changed: false,
  api_error_contract_preserved: true,
  order_change_graphql_callsites: 3,
  shipping_profile_graphql_callsites: 7,
  non_graphql_passthrough_preserved: true,
  severity_classification_preserved: true,
  public_messages_preserved: true,
  complete_graphql_error_logged: false,
  parsed_error_payload_logged: false,
  raw_tenant_id_logged: false,
  error_shape_only: true,
  tenant_shape_only: true,
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
  "wasm_compile_proven",
  "hydrate_compile_proven",
  "ssr_compile_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

if (
  review.status !==
  "commerce_admin_graphql_transport_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}
requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(doc, "complete GraphQL transport error is not logged", `${paths.doc}: policy`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce Admin GraphQL transport error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce Admin shared GraphQL diagnostics retain only error and tenant shape; execution evidence remains open",
);
