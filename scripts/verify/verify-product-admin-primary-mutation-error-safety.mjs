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
  safety: "crates/rustok-product/admin/src/transport/graphql_error_safety.rs",
  legacy: "crates/rustok-product/admin/src/transport.rs",
  graphql: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
  graphqlHttp: "crates/rustok-graphql/src/lib.rs",
  ui: "crates/rustok-product/admin/src/ui/leptos.rs",
  primaryReadGuard: "scripts/verify/verify-product-admin-primary-read-error-safety.mjs",
  categoryReadGuard: "scripts/verify/verify-product-admin-category-read-error-safety.mjs",
  readDiagnosticGuard:
    "scripts/verify/verify-product-admin-graphql-read-diagnostic-safety.mjs",
  fallbackMutationGuard:
    "scripts/verify/verify-product-admin-fallback-mutation-error-safety.mjs",
  evidence:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source.json",
  review:
    "crates/rustok-product/contracts/evidence/admin-primary-graphql-mutation-error-safety-source-review.json",
  doc: "crates/rustok-product/docs/admin-primary-graphql-mutation-error-safety.md",
  masterPlan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const facade = read(paths.facade);
const safety = read(paths.safety);
const legacy = read(paths.legacy);
const graphql = read(paths.graphql);
const graphqlHttp = read(paths.graphqlHttp);
const ui = read(paths.ui);
const primaryReadGuard = read(paths.primaryReadGuard);
const categoryReadGuard = read(paths.categoryReadGuard);
const readDiagnosticGuard = read(paths.readDiagnosticGuard);
const fallbackMutationGuard = read(paths.fallbackMutationGuard);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const masterPlan = read(paths.masterPlan);

requireText(facade, "ProductDetail, ProductDraft,", `${paths.facade}: ProductDraft wrapper import`);
requireText(
  facade,
  "graphql_error_safety::GraphqlMutationContext",
  `${paths.facade}: mutation policy wiring`,
);

const operations = [
  {
    name: "create_product",
    start: "pub(crate) async fn create_product(",
    end: "pub(crate) async fn update_product(",
    context: "GraphqlMutationContext::for_create_product(",
    call: "legacy::create_product(token, tenant_slug, tenant_id, user_id, draft)",
  },
  {
    name: "update_product",
    start: "pub(crate) async fn update_product(",
    end: "pub(crate) async fn change_product_status(",
    context: "GraphqlMutationContext::for_update_product(",
    call: "legacy::update_product(token, tenant_slug, tenant_id, user_id, id, draft)",
  },
  {
    name: "change_product_status",
    start: "pub(crate) async fn change_product_status(",
    end: "pub(crate) async fn delete_product(",
    context: "GraphqlMutationContext::for_change_product_status(",
    call: "legacy::change_product_status(token, tenant_slug, tenant_id, user_id, id, status)",
  },
];

for (const operation of operations) {
  const block = between(facade, operation.start, operation.end, paths.facade);
  for (const marker of [
    operation.context,
    operation.call,
    ".map_err(|mutation_error| context.map_error(mutation_error))",
  ]) {
    requireText(block, marker, `${paths.facade}: ${operation.name} final boundary`);
  }
  const contextIndex = block.indexOf(operation.context);
  const callIndex = block.indexOf(operation.call);
  if (!(contextIndex >= 0 && callIndex >= 0 && contextIndex < callIndex)) {
    failures.push(`${paths.facade}: ${operation.name} context must precede the GraphQL call`);
  }
}

const deleteStart = facade.indexOf("pub(crate) async fn delete_product(");
const deleteBlock = deleteStart < 0 ? "" : facade.slice(deleteStart);
for (const marker of [
  "GraphqlMutationContext::for_delete_product(",
  "legacy::delete_product(token, tenant_slug, tenant_id, user_id, id)",
  ".map_err(|mutation_error| context.map_error(mutation_error))",
]) {
  requireText(deleteBlock, marker, `${paths.facade}: delete_product final boundary`);
}
if (
  deleteBlock.indexOf("GraphqlMutationContext::for_delete_product(") >
  deleteBlock.indexOf("legacy::delete_product(")
) {
  failures.push(`${paths.facade}: delete_product context must precede the GraphQL call`);
}
if (countText(facade, ".map_err(|mutation_error| context.map_error(mutation_error))") !== 4) {
  failures.push(`${paths.facade}: expected exactly four primary mutation mappers`);
}

const mutationBlock = between(
  safety,
  "pub(super) struct GraphqlMutationContext",
  "fn text_length",
  paths.safety,
);
for (const [marker, label] of [
  ["const PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY", "mutation boundary"],
  ['"product_admin_primary_graphql_mutations"', "mutation boundary identity"],
  ["pub(super) struct GraphqlMutationContext", "private mutation context"],
  ['"product-admin-mutation:{operation}:{}"', "mutation correlation namespace"],
  ["pub(super) fn for_create_product(", "create context"],
  ["pub(super) fn for_update_product(", "update context"],
  ["pub(super) fn for_change_product_status(", "status context"],
  ["pub(super) fn for_delete_product(", "delete context"],
  ["pub(super) fn map_error(&self, error: GraphqlHttpError)", "typed mutation mapper"],
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
  ["let error_payload_length = match &error", "bounded payload derivation"],
  ["GraphqlHttpError::Http(value) | GraphqlHttpError::Graphql(value)", "payload variants"],
  ["Some(value.chars().count())", "payload character length"],
  ["GraphqlHttpError::Network | GraphqlHttpError::Unauthorized => None", "payload-free variants"],
  ["let error_payload_present = error_payload_length.is_some_and(|length| length > 0);", "payload presence"],
  ["error_payload_present,", "payload presence diagnostic"],
  ["error_payload_length = ?error_payload_length", "payload length diagnostic"],
  ["correlation_id = %self.correlation_id", "correlation diagnostics"],
  ["token_present = self.token_present", "token presence"],
  ["tenant_slug_length = ?self.tenant_slug_length", "tenant slug shape"],
  ["tenant_id_length = self.tenant_id_length", "tenant ID shape"],
  ["actor_id_length = self.actor_id_length", "actor ID shape"],
  ["resource_id_length = ?self.resource_id_length", "resource ID shape"],
  ["status_length = ?self.status_length", "status shape"],
  ["draft_present = self.draft_present", "draft presence"],
  ["boundary = PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY", "mutation boundary log"],
]) {
  requireText(mutationBlock, marker, `${paths.safety}: ${label}`);
}
if (countText(mutationBlock, "error_payload_present,") !== 2) {
  failures.push(`${paths.safety}: both mutation severity branches must record payload presence`);
}
if (countText(mutationBlock, "error_payload_length = ?error_payload_length") !== 2) {
  failures.push(`${paths.safety}: both mutation severity branches must record payload length`);
}
for (const marker of [
  "raw_error = ?error",
  "raw_error = %error",
  "error = ?error",
  "error = %error",
  "internal_message = %",
  "parsed_error = ?",
]) {
  forbidText(mutationBlock, marker, `${paths.safety}: complete mutation error payload`);
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
  "status = %",
  "status = ?",
  "draft = %",
  "draft = ?",
]) {
  forbidText(mutationBlock, marker, `${paths.safety}: raw mutation value`);
}

for (const marker of [
  "graphql_adapter::create_product(token, tenant_slug, tenant_id, user_id, draft).await",
  "graphql_adapter::update_product(token, tenant_slug, tenant_id, user_id, id, draft).await",
  "graphql_adapter::change_product_status(token, tenant_slug, tenant_id, user_id, id, status).await",
  "graphql_adapter::delete_product(token, tenant_slug, tenant_id, user_id, id).await",
]) {
  requireText(legacy, marker, `${paths.legacy}: preserved private mutation delegation`);
}
for (const marker of [
  "const CREATE_PRODUCT_MUTATION",
  "const UPDATE_PRODUCT_MUTATION",
  "const DELETE_PRODUCT_MUTATION",
  "pub(super) async fn create_product(",
  "pub(super) async fn update_product(",
  "pub(super) async fn change_product_status(",
  "pub(super) async fn delete_product(",
  "build_create_product_input(draft)",
  "status: Some(status.to_string())",
  "extra: ProductIdVariables { id }",
]) {
  requireText(graphql, marker, `${paths.graphql}: preserved mutation contract`);
}
for (const marker of [
  "pub enum GraphqlHttpError",
  "Graphql(String)",
  "Http(String)",
  "Unauthorized",
]) {
  requireText(graphqlHttp, marker, `${paths.graphqlHttp}: typed GraphQL HTTP contract`);
}
for (const marker of [
  "transport::create_product(",
  "transport::update_product(",
  "transport::change_product_status(",
  "transport::delete_product(",
]) {
  requireText(ui, marker, `${paths.ui}: preserved UI mutation composition`);
}
for (const [source, marker, label] of [
  [primaryReadGuard, "verify-product-admin-graphql-read-diagnostic-safety.mjs", "primary read compatibility guard"],
  [categoryReadGuard, "verify-product-admin-graphql-read-diagnostic-safety.mjs", "category read compatibility guard"],
  [readDiagnosticGuard, "Product Admin GraphQL read diagnostic-safety verification failed:", "shared read diagnostic guard"],
  [fallbackMutationGuard, "GraphqlFallbackMutationContext", "fallback mutation guard"],
]) {
  requireText(source, marker, `${label}: prior focused guard remains present`);
}

if (evidence.schema_version !== 1) failures.push(`${paths.evidence}: schema_version must be 1`);
if (
  evidence.status !==
  "product_admin_primary_graphql_mutation_error_safety_source_unvalidated"
) {
  failures.push(`${paths.evidence}: status mismatch`);
}
if (
  JSON.stringify(evidence.operations) !==
  JSON.stringify([
    "create_product",
    "update_product",
    "change_product_status",
    "delete_product",
  ])
) {
  failures.push(`${paths.evidence}: operation scope drift`);
}
for (const [key, expected] of Object.entries({
  context_before_graphql_call: true,
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
  result_types_changed: false,
  graphql_documents_changed: false,
  graphql_variables_changed: false,
  mutation_input_mapping_changed: false,
  owner_policy_changed: false,
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
  "read_verifiers_run",
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
  "product_admin_primary_graphql_mutation_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: status mismatch`);
}
requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: source status`);
requireText(doc, "Product admin service is temporarily unavailable", `${paths.doc}: HTTP policy`);
requireText(doc, "Product admin request could not be completed", `${paths.doc}: GraphQL policy`);
requireText(doc, "status-only update behavior", `${paths.doc}: preserved status behavior`);
requireText(doc, "payload presence and character length", `${paths.doc}: bounded payload policy`);
requireText(doc, "complete typed error is not logged", `${paths.doc}: no complete payload claim`);
requireText(
  masterPlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.masterPlan}: broad ecommerce mapper cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Product Admin primary GraphQL mutation error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Product Admin primary GraphQL mutations retain typed classification and static public errors with bounded payload-shape diagnostics; execution evidence remains open",
);
