#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  source: 'crates/rustok-commerce/src/controllers/admin/products.rs',
  evidence:
    'crates/rustok-commerce/contracts/evidence/admin-product-shipping-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-commerce/docs/admin-product-shipping-diagnostic-safety.md',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
};

const source = read(paths.source);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);
const plan = read(paths.plan);

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (content, value, expected, label) => {
  const actual = content.split(value).length - 1;
  if (actual !== expected) failures.push(`${label}: expected ${expected}, found ${actual}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const requireOrder = (content, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = content.indexOf(marker);
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
};

const diagnosticContext = between(
  source,
  'struct AdminProductShippingProfileDiagnosticContext {',
  'impl From<&AdminProductShippingProfileErrorContext>',
  'bounded shipping-profile context',
);
for (const field of [
  'tenant_id',
  'actor_id',
  'product_id',
  'shipping_profile_id',
  'operation',
]) {
  requireText(
    diagnosticContext,
    `${field}: &'static str`,
    `${paths.source}: bounded ${field}`,
  );
}
for (const value of ['Uuid', 'Option<', 'String']) {
  forbidText(diagnosticContext, value, `${paths.source}: diagnostic context storage`);
}

const conversion = between(
  source,
  'impl From<&AdminProductShippingProfileErrorContext>',
  'struct AdminProductShippingProfileDiagnosticError;',
  'shipping-profile diagnostic conversion',
);
for (const marker of [
  'tenant_id: uuid_shape(context.tenant_id)',
  'actor_id: uuid_shape(context.actor_id)',
  'product_id: optional_uuid_shape(context.product_id)',
  'shipping_profile_id: optional_uuid_shape(context.shipping_profile_id)',
  'operation: context.operation',
]) requireText(conversion, marker, `${paths.source}: diagnostic conversion`);

const diagnosticError = between(
  source,
  'struct AdminProductShippingProfileDiagnosticError;',
  'fn uuid_shape(',
  'bounded shipping-profile error',
);
for (const marker of [
  'impl std::fmt::Debug for AdminProductShippingProfileDiagnosticError',
  'formatter.write_str("redacted")',
]) requireText(diagnosticError, marker, `${paths.source}: redacted error`);
for (const value of ['CommerceError', 'message:', 'source:', 'String']) {
  forbidText(diagnosticError, value, `${paths.source}: diagnostic error payload`);
}

const requiredShape = between(
  source,
  'fn uuid_shape(',
  'fn optional_uuid_shape(',
  'required UUID shape',
);
for (const marker of ['value.is_nil()', '"nil"', '"non_nil"']) {
  requireText(requiredShape, marker, `${paths.source}: required UUID shape`);
}

const optionalShape = between(
  source,
  'fn optional_uuid_shape(',
  'fn admin_product_shipping_profile_error_policy(',
  'optional UUID shape',
);
for (const marker of [
  'None => "absent"',
  'Some(value) if value.is_nil() => "present_nil"',
  'Some(_) => "present_non_nil"',
]) requireText(optionalShape, marker, `${paths.source}: optional UUID shape`);

const policy = between(
  source,
  'fn admin_product_shipping_profile_error_policy(',
  'fn adopt_admin_product_shipping_profile_error_identity(',
  'shipping-profile policy',
);
for (const marker of [
  'CommerceError::ShippingProfileNotFound(_)',
  'CommerceError::DuplicateShippingProfileSlug(_)',
  'CommerceError::Validation(_)',
  'CommerceError::Database(_)',
  'CommerceError::ProductNotFound(_)',
  'CommerceError::VariantNotFound(_)',
  'CommerceError::DuplicateHandle { .. }',
  'CommerceError::DuplicateSku(_)',
  'CommerceError::InvalidPrice(_)',
  'CommerceError::InsufficientInventory { .. }',
  'CommerceError::InvalidOptionCombination',
  'CommerceError::NoVariants',
  'CommerceError::CannotDeletePublished',
  'CommerceError::Rich(_)',
  'CommerceError::Core(_)',
  'StatusCode::NOT_FOUND',
  'StatusCode::CONFLICT',
  'StatusCode::BAD_REQUEST',
  'StatusCode::SERVICE_UNAVAILABLE',
  'StatusCode::INTERNAL_SERVER_ERROR',
  '"commerce_admin_not_found"',
  '"commerce_admin_shipping_profile_conflict"',
  '"commerce_admin_shipping_profile_invalid"',
  '"commerce_admin_shipping_profile_storage_unavailable"',
  '"commerce_admin_shipping_profile_failed"',
  '"Commerce resource not found"',
  '"A shipping profile with this slug already exists"',
  '"Shipping profile request is invalid"',
  '"Shipping profile storage is temporarily unavailable"',
  '"Shipping profile operation could not be completed safely"',
]) requireText(policy, marker, `${paths.source}: preserved policy`);

const adoption = between(
  source,
  'fn adopt_admin_product_shipping_profile_error_identity(',
  'fn map_admin_product_shipping_profile_error(',
  'shipping-profile identity adoption',
);
for (const marker of [
  'if let CommerceError::ShippingProfileNotFound(id) = error',
  'context.shipping_profile_id = Some(*id);',
]) requireText(adoption, marker, `${paths.source}: profile identity adoption`);

const mapper = between(
  source,
  'fn map_admin_product_shipping_profile_error(',
  'async fn validate_admin_product_shipping_profile_input(',
  'shipping-profile mapper',
);
requireOrder(
  mapper,
  [
    'adopt_admin_product_shipping_profile_error_identity(&mut context, &error);',
    'admin_product_shipping_profile_error_policy(&error);',
    'let context = AdminProductShippingProfileDiagnosticContext::from(&context);',
    'let error = AdminProductShippingProfileDiagnosticError;',
    'tracing::error!(',
    'HttpError::new(status, code, message)',
  ],
  `${paths.source}: identity, policy, and shadowing order`,
);
for (const marker of [
  'error = ?error',
  'owner = ADMIN_PRODUCT_SHIPPING_PROFILE_OWNER',
  'tenant_id = %context.tenant_id',
  'actor_id = %context.actor_id',
  'product_id = %context.product_id',
  'shipping_profile_id = %context.shipping_profile_id',
  'operation = %context.operation',
  'error_kind,',
  'public_code = code',
  'status = %status',
  'boundary = ADMIN_PRODUCT_SHIPPING_PROFILE_BOUNDARY',
  '"commerce admin product shipping-profile validation failed"',
]) requireText(mapper, marker, `${paths.source}: bounded log site`);
for (const value of ['error.to_string()', 'format!(', 'error.message']) {
  forbidText(mapper, value, `${paths.source}: raw mapper payload`);
}

const validator = between(
  source,
  'async fn validate_admin_product_shipping_profile_input(',
  '/// List admin ecommerce products',
  'shipping-profile validator',
);
for (const marker of [
  'shipping_profile_slug.and_then(normalize_shipping_profile_slug)',
  'return Ok(());',
  'ShippingProfileService::new(db.clone())',
  '.ensure_shipping_profile_slug_exists(context.tenant_id, &slug)',
  '.map_err(|error| map_admin_product_shipping_profile_error(context, error))?;',
]) requireText(validator, marker, `${paths.source}: preserved validator`);

for (const marker of [
  'Permission::PRODUCTS_CREATE',
  'Permission::PRODUCTS_UPDATE',
  '"create_product_shipping_profile_validation"',
  '"update_product_shipping_profile_validation"',
  '.create_product(tenant.id, auth.user_id, input)',
  '.update_product(tenant.id, auth.user_id, id, input)',
  'Ok((StatusCode::CREATED, Json(product)))',
  'Ok(Json(product))',
]) requireText(source, marker, `${paths.source}: preserved routes`);
requireCount(
  source,
  'validate_admin_product_shipping_profile_input(',
  3,
  'one validator and two route callsites',
);
requireCount(
  source,
  'map_admin_product_shipping_profile_error(',
  2,
  'one mapper and one validator handoff',
);

if (
  evidence.status !==
  'commerce_admin_product_shipping_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  raw_commerce_error_logged: false,
  raw_tenant_uuid_logged: false,
  raw_actor_uuid_logged: false,
  raw_product_uuid_logged: false,
  raw_shipping_profile_uuid_logged: false,
  redacted_error_debug_logged: true,
  required_uuid_shapes_logged: true,
  optional_uuid_shapes_logged: true,
  shipping_profile_not_found_identity_adoption_preserved: true,
  typed_policy_selection_precedes_shadowing: true,
  http_policy_preserved: true,
  validator_preserved: true,
  create_and_update_callsites_preserved: true,
  permissions_preserved: true,
  catalog_owner_calls_preserved: true,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'shipping-profile-not-found identity adoption and HTTP policy selection',
  'Debug output is always `redacted`',
  'The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.',
]) requireText(doc, marker, `${paths.doc}: documentation contract`);
requireText(
  plan,
  'Finish correlation-safe mapper cleanup',
  `${paths.plan}: broad cleanup remains open`,
);

if (failures.length > 0) {
  console.error('Commerce admin product shipping diagnostic verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  'Commerce admin product shipping-profile diagnostics are bounded while typed policy, identity adoption, validation, permissions, and catalog calls remain unchanged; execution validation remains open',
);
