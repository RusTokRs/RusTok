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

const paths = {
  source: "crates/rustok-commerce/src/controllers/products.rs",
  adminSource: "crates/rustok-commerce/src/controllers/admin/products.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-product-diagnostic-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-product-diagnostic-safety.md",
  plan: "crates/rustok-commerce/docs/implementation-plan.md",
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const actual = source.split(value).length - 1;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};

function blockBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const source = read(paths.source);
const adminSource = read(paths.adminSource);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const diagnosticContext = blockBetween(
  source,
  "struct AdminProductDiagnosticContext {",
  "impl From<&AdminProductErrorContext> for AdminProductDiagnosticContext {",
  "bounded product context",
);
for (const field of ["tenant_id", "actor_id", "product_id", "operation"]) {
  requireText(diagnosticContext, `${field}: &'static str`, `${paths.source}: bounded ${field}`);
}
for (const forbidden of ["Uuid", "Option<", "String"]) {
  forbidText(diagnosticContext, forbidden, `${paths.source}: diagnostic context storage`);
}

const contextConversion = blockBetween(
  source,
  "impl From<&AdminProductErrorContext> for AdminProductDiagnosticContext {",
  "struct AdminProductDiagnosticError;",
  "product context conversion",
);
for (const marker of [
  "tenant_id: uuid_shape(context.tenant_id)",
  "actor_id: uuid_shape(context.actor_id)",
  "product_id: optional_uuid_shape(context.product_id)",
  "operation: context.operation",
]) requireText(contextConversion, marker, `${paths.source}: diagnostic context conversion`);

const diagnosticError = blockBetween(
  source,
  "struct AdminProductDiagnosticError;",
  "fn uuid_shape(",
  "bounded product error",
);
for (const marker of [
  "impl std::fmt::Debug for AdminProductDiagnosticError",
  'formatter.write_str("redacted")',
]) requireText(diagnosticError, marker, `${paths.source}: redacted diagnostic error`);
for (const forbidden of ["CommerceError", "message:", "source:", "String"]) {
  forbidText(diagnosticError, forbidden, `${paths.source}: diagnostic error payload`);
}

const requiredShape = blockBetween(
  source,
  "fn uuid_shape(",
  "fn optional_uuid_shape(",
  "required UUID shape",
);
for (const marker of ["value.is_nil()", '"nil"', '"non_nil"']) {
  requireText(requiredShape, marker, `${paths.source}: required UUID shape`);
}

const optionalShape = blockBetween(
  source,
  "fn optional_uuid_shape(",
  "fn product_error_policy(",
  "optional UUID shape",
);
for (const marker of [
  'None => "absent"',
  'Some(value) if value.is_nil() => "present_nil"',
  'Some(_) => "present_non_nil"',
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

const policy = blockBetween(
  source,
  "fn product_error_policy(",
  "fn adopt_product_error_identity(",
  "admin product policy",
);
for (const marker of [
  "CommerceError::Database(_)",
  "CommerceError::ProductNotFound(_)",
  "CommerceError::DuplicateHandle { .. }",
  "CommerceError::DuplicateSku(_)",
  "CommerceError::Validation(_) | CommerceError::NoVariants",
  "CommerceError::CannotDeletePublished",
  "CommerceError::Core(_)",
  '"commerce_admin_product_storage_unavailable"',
  '"commerce_admin_not_found"',
  '"commerce_admin_product_handle_conflict"',
  '"commerce_admin_product_sku_conflict"',
  '"commerce_admin_product_invalid"',
  '"commerce_admin_product_state_conflict"',
  '"commerce_admin_product_failed"',
]) requireText(policy, marker, `${paths.source}: preserved product policy`);

const identity = blockBetween(
  source,
  "fn adopt_product_error_identity(",
  "pub(crate) fn map_admin_product_error(",
  "product identity adoption",
);
for (const marker of [
  "CommerceError::ProductNotFound(id)",
  "context.product_id = Some(*id)",
]) requireText(identity, marker, `${paths.source}: product identity adoption`);

const mapper = blockBetween(
  source,
  "pub(crate) fn map_admin_product_error(",
  "/// Shared admin product list handler.",
  "admin product mapper",
);
requireOrder(
  mapper,
  [
    "adopt_product_error_identity(&mut context, &error);",
    "let (status, code, message, error_kind) = product_error_policy(&error);",
    "let context = AdminProductDiagnosticContext::from(&context);",
    "let error = AdminProductDiagnosticError;",
    "tracing::error!(",
    "HttpError::new(status, code, message)",
  ],
  `${paths.source}: typed policy and shadowing order`,
);
for (const marker of [
  "error = ?error",
  "owner = ADMIN_PRODUCT_OWNER",
  "tenant_id = %context.tenant_id",
  "actor_id = %context.actor_id",
  "product_id = ?context.product_id",
  "operation = %context.operation",
  "error_kind,",
  "public_code = code",
  "status = %status",
  "boundary = ADMIN_PRODUCT_BOUNDARY",
  '"commerce admin product operation failed"',
]) requireText(mapper, marker, `${paths.source}: bounded product log site`);
for (const forbidden of ["error.to_string()", "error.message", "format!("]) {
  forbidText(mapper, forbidden, `${paths.source}: raw product diagnostic payload`);
}

requireCount(source, "map_admin_product_error(", 9, "one mapper and eight shared callsites");
requireCount(adminSource, "map_admin_product_error(", 2, "two admin write callsites");
for (const marker of [
  "Permission::PRODUCTS_LIST",
  "Permission::PRODUCTS_READ",
  "Permission::PRODUCTS_DELETE",
  "Permission::PRODUCTS_UPDATE",
  '"list_products_count"',
  '"list_products_page"',
  '"list_product_translations"',
  '"list_product_tags"',
  '"show_product"',
  '"delete_product"',
  '"publish_product"',
  '"unpublish_product"',
  ".get_product_with_locale_fallback(",
  ".delete_product(tenant.id, auth.user_id, id)",
  ".publish_product(tenant.id, auth.user_id, id)",
  ".unpublish_product(tenant.id, auth.user_id, id)",
  "product_translation_title_search_condition(",
  "metrics::record_read_path_query(",
  "metrics::record_read_path_budget(",
  "PaginationMeta::new(pagination.page, pagination.limit(), total)",
]) requireText(source, marker, `${paths.source}: preserved route contract`);

for (const marker of [
  "Permission::PRODUCTS_CREATE",
  "Permission::PRODUCTS_UPDATE",
  '"create_product"',
  '"update_product"',
  ".create_product(tenant.id, auth.user_id, input)",
  ".update_product(tenant.id, auth.user_id, id, input)",
  "validate_admin_product_shipping_profile_input(",
  "map_admin_product_shipping_profile_error(",
]) requireText(adminSource, marker, `${paths.adminSource}: preserved admin wrapper contract`);

if (
  evidence.status !==
  "commerce_admin_product_diagnostic_safety_source_reviewed_unvalidated"
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_commerce_error_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_product_uuid_logged: false,
  redacted_error_debug_logged: true,
  required_uuid_shapes_logged: true,
  optional_uuid_shapes_logged: true,
  operation_preserved: true,
  product_not_found_identity_adoption_preserved: true,
  typed_policy_selection_precedes_shadowing: true,
  product_error_policy_preserved: true,
  http_envelopes_preserved: true,
  ten_shared_mapper_callsites_preserved: true,
  permissions_preserved: true,
  filters_locale_metrics_pagination_preserved: true,
  shipping_profile_validation_mapper_unchanged: true,
  broad_ecommerce_cleanup_closed: false,
  runtime_evidence_claimed: false,
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
  "compile_proven",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "product-not-found identity adoption and HTTP policy selection",
  "Debug output is always `redacted`",
  "The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.",
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  "Finish correlation-safe mapper cleanup",
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce admin product diagnostic verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce admin product diagnostics are bounded while typed identity adoption, ten owner callsites, permissions, filters, metrics, and HTTP policies remain unchanged; execution validation remains open",
);
