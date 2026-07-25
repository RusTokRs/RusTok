#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const adminProducts = read('crates/rustok-commerce/src/controllers/admin/products.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
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

const policy = between(
  adminProducts,
  'fn admin_product_shipping_profile_error_policy(',
  'fn adopt_admin_product_shipping_profile_error_identity(',
  'shipping-profile policy',
);
const identityAdoption = between(
  adminProducts,
  'fn adopt_admin_product_shipping_profile_error_identity(',
  'fn map_admin_product_shipping_profile_error(',
  'shipping-profile identity adoption',
);
const mapper = between(
  adminProducts,
  'fn map_admin_product_shipping_profile_error(',
  'async fn validate_admin_product_shipping_profile_input(',
  'shipping-profile mapper',
);
const validator = between(
  adminProducts,
  'async fn validate_admin_product_shipping_profile_input(',
  '/// List admin ecommerce products',
  'shipping-profile validator',
);
const createRoute = between(
  adminProducts,
  'pub async fn create_product(',
  '/// Show admin ecommerce product',
  'create product route',
);
const updateRoute = between(
  adminProducts,
  'pub async fn update_product(',
  '/// Delete admin ecommerce product',
  'update product route',
);

for (const [value, label] of [
  [
    'const ADMIN_PRODUCT_SHIPPING_PROFILE_OWNER: &str = "rustok_commerce.shipping_profile";',
    'owner constant',
  ],
  [
    'const ADMIN_PRODUCT_SHIPPING_PROFILE_BOUNDARY: &str =',
    'boundary constant declaration',
  ],
  [
    '"commerce_admin_product_shipping_profile_http";',
    'boundary constant value',
  ],
  ['type AdminProductShippingProfileHttpPolicy = (', 'static policy type'],
  [
    'struct AdminProductShippingProfileErrorContext {',
    'typed validation context',
  ],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['product_id: Option<Uuid>,', 'product context field'],
  ['shipping_profile_id: Option<Uuid>,', 'profile context field'],
  ["operation: &'static str,", 'operation context field'],
  ['fn map_admin_product_shipping_profile_error(', 'typed mapper'],
  [
    'async fn validate_admin_product_shipping_profile_input(',
    'local validation helper',
  ],
]) requireText(adminProducts, value, label);

for (const [value, label] of [
  ['CommerceError::ShippingProfileNotFound(_)', 'profile not-found variant'],
  [
    'CommerceError::DuplicateShippingProfileSlug(_)',
    'duplicate profile slug variant',
  ],
  ['CommerceError::Validation(_)', 'validation variant'],
  ['CommerceError::Database(_)', 'database variant'],
  ['CommerceError::ProductNotFound(_)', 'unexpected product variant'],
  ['CommerceError::VariantNotFound(_)', 'unexpected variant variant'],
  ['CommerceError::DuplicateHandle { .. }', 'unexpected duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'unexpected duplicate SKU variant'],
  ['CommerceError::InvalidPrice(_)', 'unexpected invalid price variant'],
  [
    'CommerceError::InsufficientInventory { .. }',
    'unexpected inventory variant',
  ],
  [
    'CommerceError::InvalidOptionCombination',
    'unexpected option combination variant',
  ],
  ['CommerceError::NoVariants', 'unexpected no-variants variant'],
  [
    'CommerceError::CannotDeletePublished',
    'unexpected published-state variant',
  ],
  ['CommerceError::Rich(_)', 'unexpected rich variant'],
  ['CommerceError::Core(_)', 'unexpected core variant'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_admin_not_found"', 'not-found code'],
  [
    '"commerce_admin_shipping_profile_conflict"',
    'duplicate slug conflict code',
  ],
  [
    '"commerce_admin_shipping_profile_invalid"',
    'validation code',
  ],
  [
    '"commerce_admin_shipping_profile_storage_unavailable"',
    'storage code',
  ],
  ['"commerce_admin_shipping_profile_failed"', 'fail-closed code'],
  ['"Commerce resource not found"', 'static not-found message'],
  [
    '"A shipping profile with this slug already exists"',
    'static duplicate slug message',
  ],
  [
    '"Shipping profile request is invalid"',
    'static validation message',
  ],
  [
    '"Shipping profile storage is temporarily unavailable"',
    'static storage message',
  ],
  [
    '"Shipping profile operation could not be completed safely"',
    'static fail-closed message',
  ],
]) requireText(policy, value, label);

for (const [value, label] of [
  [
    'if let CommerceError::ShippingProfileNotFound(id) = error',
    'typed profile identity variant',
  ],
  [
    'context.shipping_profile_id = Some(*id);',
    'typed profile identity adoption',
  ],
]) requireText(identityAdoption, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed cause log'],
  ['owner = ADMIN_PRODUCT_SHIPPING_PROFILE_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['product_id = ?context.product_id', 'product identity log'],
  [
    'shipping_profile_id = ?context.shipping_profile_id',
    'profile identity log',
  ],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public code log'],
  ['status = %status', 'status log'],
  [
    'boundary = ADMIN_PRODUCT_SHIPPING_PROFILE_BOUNDARY',
    'boundary log',
  ],
  ['HttpError::new(status, code, message)', 'static envelope'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  [
    'shipping_profile_slug.and_then(normalize_shipping_profile_slug)',
    'slug normalization',
  ],
  ['return Ok(());', 'absent slug no-op'],
  ['ShippingProfileService::new(db.clone())', 'profile service construction'],
  [
    '.ensure_shipping_profile_slug_exists(context.tenant_id, &slug)',
    'owner validation call',
  ],
  [
    '.map_err(|error| map_admin_product_shipping_profile_error(context, error))?;',
    'context-aware error mapping',
  ],
]) requireText(validator, value, label);

for (const [
  block,
  permission,
  productIdentity,
  operation,
  serviceCall,
  response,
  label,
] of [
  [
    createRoute,
    'Permission::PRODUCTS_CREATE',
    'None,',
    '"create_product_shipping_profile_validation"',
    '.create_product(tenant.id, auth.user_id, input)',
    'Ok((StatusCode::CREATED, Json(product)))',
    'create route',
  ],
  [
    updateRoute,
    'Permission::PRODUCTS_UPDATE',
    'Some(id)',
    '"update_product_shipping_profile_validation"',
    '.update_product(tenant.id, auth.user_id, id, input)',
    'Ok(Json(product))',
    'update route',
  ],
]) {
  requireText(block, permission, `${label} permission`);
  requireText(
    block,
    'validate_admin_product_shipping_profile_input(',
    `${label} local validator`,
  );
  requireText(
    block,
    'AdminProductShippingProfileErrorContext::new(',
    `${label} typed context`,
  );
  requireText(block, 'tenant.id', `${label} tenant identity`);
  requireText(block, 'auth.user_id', `${label} actor identity`);
  requireText(block, productIdentity, `${label} product identity`);
  requireText(block, operation, `${label} operation`);
  requireText(
    block,
    'input.shipping_profile_slug.as_deref()',
    `${label} profile slug forwarding`,
  );
  requireText(block, serviceCall, `${label} catalog service contract`);
  requireText(block, response, `${label} response contract`);
}

const validationUses =
  adminProducts.match(
    /validate_admin_product_shipping_profile_input\(\s+runtime\.db\(\),\s+AdminProductShippingProfileErrorContext::new\(/g,
  ) ?? [];
if (validationUses.length !== 2) {
  failures.push(
    `expected two context-aware product shipping-profile validation callsites, found ${validationUses.length}`,
  );
}

for (const value of [
  'super::validate_product_shipping_profile_input(',
  'super::map_shipping_profile_error(',
  'err.to_string()',
  'error.to_string()',
  'other.to_string()',
  'format!("Shipping profile',
]) forbidText(adminProducts, value, 'stale or unsafe product shipping-profile mapping');

if (failures.length > 0) {
  console.error(
    'Commerce admin product shipping-profile error-context verification failed:',
  );
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin product shipping-profile validation retains typed causes and route context',
);
