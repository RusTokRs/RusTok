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
  facade: "crates/rustok-product/admin/src/catalog_transport.rs",
  wrappers:
    "crates/rustok-product/admin/src/transport/graphql_fallback_mutations.rs",
  safety:
    "crates/rustok-product/admin/src/transport/graphql_fallback_mutation_error_safety.rs",
  legacy: "crates/rustok-product/admin/src/transport.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  primaryMutationGuard:
    "scripts/verify/verify-product-admin-primary-mutation-error-safety.mjs",
  categoryReadGuard:
    "scripts/verify/verify-product-admin-category-read-error-safety.mjs",
  primaryReadGuard:
    "scripts/verify/verify-product-admin-primary-read-error-safety.mjs",
  catalogOptionsGuard:
    "scripts/verify/verify-product-admin-catalog-options-error-safety.mjs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-fallback-graphql-mutation-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-fallback-graphql-mutation-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-fallback-graphql-mutation-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const facade = read(paths.facade);
const wrappers = read(paths.wrappers);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const graphql = read(paths.graphql);
const primaryMutationGuard = read(paths.primaryMutationGuard);
const categoryReadGuard = read(paths.categoryReadGuard);
const primaryReadGuard = read(paths.primaryReadGuard);
const catalogOptionsGuard = read(paths.catalogOptionsGuard);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const masterPlan = read(paths.masterPlan);

const operationNames = [
  "create_product_attribute",
  "create_product_attribute_option",
  "create_catalog_category",
  "create_attribute_schema",
  "set_category_schema_mode",
  "create_product_attribute_schema_group",
  "create_category_attribute_group",
  "bind_schema_attribute",
  "bind_category_attribute",
  "save_product_attribute_values",
  "clear_detached_product_attribute_values",
];

for (const marker of [
  '#[path = "transport/graphql_fallback_mutation_error_safety.rs"]',
  "mod graphql_fallback_mutation_error_safety;",
  '#[path = "transport/graphql_fallback_mutations.rs"]',
  "mod graphql_fallback_mutations;",
  "pub(crate) use graphql_fallback_mutations::{",
]) {
  requireText(facade, marker, `${paths.facade}: explicit sanitized fallback exports`);
}
for (const name of operationNames) {
  requireText(facade, name, `${paths.facade}: explicit export ${name}`);
}

for (let index = 0; index < operationNames.length; index += 1) {
  const name = operationNames[index];
  const start = `pub(crate) async fn ${name}(`;
  const next = operationNames[index + 1];
  const from = wrappers.indexOf(start);
  const block = next
    ? between(wrappers, start, `pub(crate) async fn ${next}(`, paths.wrappers)
    : from < 0
      ? ""
      : wrappers.slice(from);
  if (from < 0) failures.push(`${paths.wrappers}: missing ${start}`);

  const context = `GraphqlFallbackMutationContext::for_${name}(`;
  const call = `legacy::${name}(`;
  const mapper =
    ".map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))";
  for (const marker of [context, call, mapper]) {
    requireText(block, marker, `${paths.wrappers}: ${name} final boundary`);
  }
  if (!(block.indexOf(context) >= 0 && block.indexOf(context) < block.indexOf(call))) {
    failures.push(`${paths.wrappers}: ${name} context must precede compatibility execution`);
  }
}

const fallbackMapper =
  ".map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))";
if (countText(wrappers, fallbackMapper) !== operationNames.length) {
  failures.push(`${paths.wrappers}: expected exactly eleven final fallback mappers`);
}

for (const [marker, label] of [
  ["pub(super) struct GraphqlFallbackMutationContext", "private typed context"],
  ["Uuid::new_v4()", "unique correlation id"],
  ['"product-admin-fallback-mutation:{operation}:{}"', "correlation namespace"],
  ['"product_admin_fallback_graphql_mutations"', "dedicated boundary"],
  ["pub(super) fn map_error(&self, error: GraphqlHttpError)", "typed mapper"],
  ["GraphqlHttpError::Network", "network classification"],
  ["GraphqlHttpError::Http(_)", "HTTP classification"],
  ["GraphqlHttpError::Unauthorized", "authentication classification"],
  ["GraphqlHttpError::Graphql(_)", "GraphQL classification"],
  ['"Product admin service is temporarily unavailable"', "HTTP public message"],
  ['"Product admin request could not be completed"', "GraphQL public message"],
  ['"product.admin_graphql_network_unavailable"', "network code"],
  ['"product.admin_graphql_http_unavailable"', "HTTP code"],
  ['"product.admin_graphql_authentication_required"', "auth code"],
  ['"product.admin_graphql_request_rejected"', "GraphQL code"],
  ["let error_payload_length = match &error", "payload shape derivation"],
  [
    "GraphqlHttpError::Http(value) | GraphqlHttpError::Graphql(value)",
    "payload-bearing variants",
  ],
  ["Some(value.chars().count())", "payload character length"],
  [
    "GraphqlHttpError::Network | GraphqlHttpError::Unauthorized => None",
    "payload-free variants",
  ],
  [
    "let error_payload_present = error_payload_length.is_some_and(|length| length > 0);",
    "payload presence",
  ],
  ["error_payload_present,", "bounded payload presence diagnostic"],
  ["error_payload_length = ?error_payload_length", "bounded payload length diagnostic"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["token_present = self.token_present", "token presence"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant slug shape"],
  ["tenant_id_length = self.tenant_id_length", "tenant ID shape"],
  ["actor_id_length = self.actor_id_length", "actor ID shape"],
  ["resource_id_length = ?self.resource_id_length", "resource ID shape"],
  ["locale_length = ?self.locale_length", "locale shape"],
  ["item_count = ?self.item_count", "collection count"],
  ["input_present = self.input_present", "input presence"],
  ["native_fallback_attempted = self.native_fallback_attempted", "fallback state"],
  ["native_fallback_attempted: true", "final error fallback invariant"],
  ["public_error", "static typed return"],
]) {
  requireText(safety, marker, `${paths.safety}: ${label}`);
}
if (countText(safety, "error_payload_present,") !== 2) {
  failures.push(`${paths.safety}: both severity branches must emit payload presence`);
}
if (countText(safety, "error_payload_length = ?error_payload_length") !== 2) {
  failures.push(`${paths.safety}: both severity branches must emit payload length`);
}
for (const name of operationNames) {
  requireText(safety, `pub(super) fn for_${name}(`, `${paths.safety}: context ${name}`);
}

for (const marker of [
  "raw_error = ?error",
  "raw_error = %error",
  "error = ?error",
  "error = %error",
  "internal_message = %",
  "parsed_error = ?",
]) {
  forbidText(safety, marker, `${paths.safety}: complete typed error payload`);
}
for (const marker of [
  "token = %",
  "token = ?",
  "tenant_slug = %",
  "tenant_slug = ?",
  "tenant_id = %",
  "tenant_id = ?",
  "actor_id = %",
  "actor_id = ?",
  "resource_id = %",
  "resource_id = ?",
  "product_id = %",
  "product_id = ?",
  "locale = %",
  "locale = ?",
  "draft = %",
  "draft = ?",
  "patches = %",
  "patches = ?",
  "attribute_ids = %",
  "attribute_ids = ?",
]) {
  forbidText(safety, marker, `${paths.safety}: raw request values must not be logged`);
}

for (let index = 0; index < operationNames.length; index += 1) {
  const name = operationNames[index];
  const next = operationNames[index + 1] ?? "update_product";
  const block = between(
    legacy,
    `pub(crate) async fn ${name}(`,
    `pub(crate) async fn ${next}(`,
    paths.legacy,
  );
  const nativeCall = `native_server_adapter::${name}(`;
  const graphqlCall = `graphql_adapter::${name}(`;
  for (const marker of [nativeCall, "Err(_) => {", graphqlCall]) {
    requireText(block, marker, `${paths.legacy}: ${name} native-first contract`);
  }
  const nativeIndex = block.indexOf(nativeCall);
  const errorIndex = block.indexOf("Err(_) => {");
  const graphqlIndex = block.indexOf(graphqlCall);
  if (!(nativeIndex >= 0 && nativeIndex < errorIndex && errorIndex < graphqlIndex)) {
    failures.push(`${paths.legacy}: ${name} native/fallback order drift`);
  }
  if (countText(block, nativeCall) !== 1 || countText(block, graphqlCall) !== 1) {
    failures.push(`${paths.legacy}: ${name} must retain one native and one GraphQL call`);
  }
}

for (const marker of [
  "CREATE_PRODUCT_ATTRIBUTE_MUTATION",
  "CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION",
  "CREATE_CATALOG_CATEGORY_MUTATION",
  "CREATE_ATTRIBUTE_SCHEMA_MUTATION",
  "SET_CATEGORY_SCHEMA_MODE_MUTATION",
  "CREATE_SCHEMA_GROUP_MUTATION",
  "CREATE_CATEGORY_GROUP_MUTATION",
  "BIND_SCHEMA_ATTRIBUTE_MUTATION",
  "BIND_CATEGORY_ATTRIBUTE_MUTATION",
  "SAVE_ATTRIBUTE_VALUES_MUTATION",
  "CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION",
  "TenantUserScopedVariables",
  "LocaleMutationVariables",
  "InputVariables",
  "SaveAttributeValuesVariables",
  "ClearDetachedAttributeValuesVariables",
]) {
  requireText(graphql, marker, `${paths.graphql}: preserved mutation contract`);
}

for (const [source, marker, label] of [
  [primaryMutationGuard, "complete typed error is not logged", "primary mutation guard"],
  [categoryReadGuard, "verify-product-admin-graphql-read-diagnostic-safety.mjs", "category read guard"],
  [primaryReadGuard, "verify-product-admin-graphql-read-diagnostic-safety.mjs", "primary read guard"],
  [catalogOptionsGuard, "raw_error_shape_only", "catalog options guard"],
]) {
  requireText(source, marker, `${label}: prior focused guard remains present`);
}

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (
  evidence.status !==
  "product_admin_fallback_graphql_mutation_error_safety_source_unvalidated"
) failures.push(`${paths.evidence}: status mismatch`);
if (JSON.stringify(evidence.operations) !== JSON.stringify(operationNames)) {
  failures.push(`${paths.evidence}: operation scope drift`);
}
for (const [key, expected] of Object.entries({
  explicit_facade_reexport: true,
  context_before_compatibility_executor: true,
  unique_correlation_id: true,
  typed_graphql_error_classification: true,
  network_static_public_envelope: true,
  http_static_public_envelope: true,
  unauthorized_static_public_envelope: true,
  graphql_static_public_envelope: true,
  raw_http_status_public: false,
  raw_graphql_message_public: false,
  complete_typed_error_logged: false,
  error_payload_shape_only: true,
  private_typed_error_classification: true,
  safe_request_shape_only: true,
  native_first_preserved: true,
  single_graphql_fallback_preserved: true,
  result_types_changed: false,
  graphql_documents_changed: false,
  graphql_variables_changed: false,
  input_mapping_changed: false,
  response_mapping_changed: false,
  retry_added: false,
  fallback_added: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
if (evidence.safe_diagnostics?.includes("private_typed_graphql_error")) {
  failures.push(`${paths.evidence}: safe_diagnostics must not retain private_typed_graphql_error`);
}
for (const marker of [
  "error_payload_present",
  "error_payload_length",
  "error_kind",
  "code",
  "boundary",
]) {
  if (!evidence.safe_diagnostics?.includes(marker)) {
    failures.push(`${paths.evidence}: safe_diagnostics must include ${marker}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "focused_verifier_run",
  "prior_verifiers_run",
  "aggregate_verifier_run",
  "broad_ecommerce_verifier_run",
  "workflow_checks_run",
  "ci_run",
  "browser_runtime_proven",
  "mounted_transport_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

if (
  review.status !==
  "product_admin_fallback_graphql_mutation_error_safety_source_reviewed_unvalidated"
) failures.push(`${paths.review}: status mismatch`);
const expectedChangedFiles = [
  paths.safety,
  paths.evidence,
  paths.review,
  paths.doc,
  "scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs",
];
if (JSON.stringify(review.changed_files_expected) !== JSON.stringify(expectedChangedFiles)) {
  failures.push(`${paths.review}: changed_files_expected drift`);
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "browser_runtime_proven",
  "mounted_transport_proven",
]) {
  if (review.validation?.[key] !== false) {
    failures.push(`${paths.review}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(review.execution) || review.execution.length !== 0) {
  failures.push(`${paths.review}: execution must remain empty`);
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(doc, "eleven Product Admin commands", `${paths.doc}: operation scope`);
requireText(doc, "native-first", `${paths.doc}: transport policy`);
requireText(doc, "payload presence and character length", `${paths.doc}: bounded payload policy`);
requireText(doc, "complete typed error is not logged", `${paths.doc}: no full payload claim`);
requireText(doc, "Product admin service is temporarily unavailable", `${paths.doc}: HTTP policy`);
requireText(doc, "Product admin request could not be completed", `${paths.doc}: GraphQL policy`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Product Admin fallback GraphQL mutation error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin native-first GraphQL fallback mutations retain one fallback and typed classification with bounded payload-shape diagnostics; execution evidence remains open",
);
