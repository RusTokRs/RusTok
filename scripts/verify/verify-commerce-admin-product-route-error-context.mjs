#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const products = read('crates/modules/rustok-commerce/src/controllers/products.rs');
const adminProducts = read('crates/modules/rustok-commerce/src/controllers/admin/products.rs');
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
  products,
  'fn product_error_policy(',
  'fn adopt_product_error_identity(',
  'admin product policy',
);
const identityAdoption = between(
  products,
  'fn adopt_product_error_identity(',
  'pub(crate) fn map_admin_product_error(',
  'admin product identity adoption',
);
const mapper = between(
  products,
  'pub(crate) fn map_admin_product_error(',
  'pub(crate) fn admin_product_command_idempotency_key(',
  'admin product mapper',
);
const listRoute = between(
  products,
  'pub async fn list_products(',
  'fn pick_product_translation',
  'admin product list route',
);
const showRoute = between(
  products,
  'pub async fn show_product(',
  '/// Shared admin product delete handler.',
  'admin product show route',
);
const deleteRoute = between(
  products,
  'pub async fn delete_product(',
  '/// Shared admin product publish handler.',
  'admin product delete route',
);
const publishRoute = between(
  products,
  'pub async fn publish_product(',
  '/// Shared admin product unpublish handler.',
  'admin product publish route',
);
const unpublishRoute = between(
  products,
  'pub async fn unpublish_product(',
  '#[derive(Debug, serde::Deserialize',
  'admin product unpublish route',
);
const createRoute = between(
  adminProducts,
  'pub async fn create_product(',
  '/// Show admin ecommerce product',
  'admin product create route',
);
const updateRoute = between(
  adminProducts,
  'pub async fn update_product(',
  '/// Delete admin ecommerce product',
  'admin product update route',
);

for (const [value, label] of [
  ['const ADMIN_PRODUCT_OWNER: &str = "rustok_product.catalog";', 'owner constant'],
  [
    'const ADMIN_PRODUCT_BOUNDARY: &str = "commerce_admin_product_http";',
    'HTTP boundary constant',
  ],
  ['type AdminProductHttpPolicy = (', 'static policy type'],
  ['pub(crate) struct AdminProductErrorContext {', 'shared typed context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['actor_id: Uuid,', 'actor context field'],
  ['product_id: Option<Uuid>,', 'product identity field'],
  ["operation: &'static str,", 'operation field'],
  ['pub(crate) fn new(', 'shared context constructor'],
  ['pub(crate) fn map_admin_product_error(', 'shared typed mapper'],
]) requireText(products, value, label);

for (const [value, label] of [
  ['CommerceError::Database(_)', 'database variant'],
  ['CommerceError::ProductNotFound(_)', 'product not-found variant'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found variant'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'duplicate SKU variant'],
  ['CommerceError::Validation(_)', 'validation variant'],
  ['CommerceError::NoVariants', 'no variants variant'],
  ['CommerceError::CannotDeletePublished', 'published-state variant'],
  ['CommerceError::Core(error)', 'classified core owner variant'],
  ['"commerce_admin_product_storage_unavailable"', 'storage code'],
  ['"commerce_admin_not_found"', 'not-found code'],
  ['"commerce_admin_product_handle_conflict"', 'handle conflict code'],
  ['"commerce_admin_product_sku_conflict"', 'SKU conflict code'],
  ['"commerce_admin_product_invalid"', 'validation code'],
  ['"commerce_admin_product_state_conflict"', 'state conflict code'],
  ['"commerce_admin_product_failed"', 'fail-closed code'],
  ['"Product storage is temporarily unavailable"', 'static storage message'],
  ['"Product request is invalid"', 'static validation message'],
  [
    '"Product operation could not be completed safely"',
    'static fail-closed message',
  ],
]) requireText(policy + products.slice(products.indexOf("pub(crate) fn product_core_http_policy("), products.indexOf("fn product_error_policy(")), value, label);

for (const [value, label] of [
  ['CommerceError::ProductNotFound(id)', 'typed product identity variant'],
  ['context.product_id = Some(*id)', 'typed product identity adoption'],
]) requireText(identityAdoption, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed cause log'],
  ['owner = ADMIN_PRODUCT_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['actor_id = %context.actor_id', 'actor log'],
  ['product_id = ?context.product_id', 'product identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_PRODUCT_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(mapper, value, label);

for (const [value, label] of [
  ['Permission::PRODUCTS_LIST', 'list permission'],
  ['let requested_limit = params', 'requested limit capture'],
  ['let pagination = params.pagination.unwrap_or_default();', 'list pagination'],
  ['.unwrap_or(request_context.locale.as_str())', 'locale fallback'],
  ['product::Column::TenantId.eq(tenant.id)', 'tenant filter'],
  ['product::Column::Status.eq(status)', 'status filter'],
  ['product::Column::Vendor.eq(vendor)', 'vendor filter'],
  ['product::Column::ProductType.eq(product_type)', 'product type filter'],
  ['product_translation_title_search_condition(', 'localized search filter'],
  ['"list_products_count"', 'count operation'],
  ['"list_products_page"', 'page operation'],
  ['"list_product_translations"', 'translations operation'],
  ['"list_product_tags"', 'tag operation'],
  ['CommerceError::Database(error)', 'typed database wrapping'],
  ['.offset(pagination.offset())', 'page offset'],
  ['.limit(pagination.limit())', 'page limit'],
  ['.load_product_tag_map(', 'tag map owner call'],
  ['pick_product_translation(items, locale, tenant.default_locale.as_str())', 'translation fallback'],
  ['metrics::record_read_path_query(', 'query metrics'],
  ['metrics::record_read_path_budget(', 'budget metrics'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'pagination response'],
]) requireText(listRoute, value, label);

for (const [block, permission, operation, serviceCall, response, label] of [
  [
    showRoute,
    'Permission::PRODUCTS_READ',
    '"show_product"',
    '.get_product_with_locale_fallback(',
    'Ok(Json(product))',
    'show route',
  ],
  [
    deleteRoute,
    'Permission::PRODUCTS_DELETE',
    '"delete_product"',
    '.delete_product(port_context.clone(), id)',
    'Ok(StatusCode::NO_CONTENT)',
    'delete route',
  ],
  [
    publishRoute,
    'Permission::PRODUCTS_UPDATE',
    '"publish_product"',
    '.publish_product(port_context.clone(), id)',
    'Ok(Json(product))',
    'publish route',
  ],
  [
    unpublishRoute,
    'Permission::PRODUCTS_UPDATE',
    '"unpublish_product"',
    '.unpublish_product(port_context.clone(), id)',
    'Ok(Json(product))',
    'unpublish route',
  ],
]) {
  requireText(block, permission, `${label} permission`);
  requireText(block, operation, `${label} operation`);
  requireText(block, 'Some(id)', `${label} product identity`);
  requireText(block, serviceCall, `${label} service contract`);
  requireText(block, block === showRoute ? 'map_admin_product_error(' : 'map_admin_product_port_error(', `${label} owner error mapper`);
  requireText(block, response, `${label} response contract`);
}

for (const [block, permission, operation, productIdentity, serviceCall, response, label] of [
  [
    createRoute,
    'Permission::PRODUCTS_CREATE',
    '"create_product"',
    'None,',
    '.product_catalog_command_port()',
    'Ok((StatusCode::CREATED, Json(product)))',
    'create route',
  ],
  [
    updateRoute,
    'Permission::PRODUCTS_UPDATE',
    '"update_product"',
    'Some(id)',
    '.product_catalog_command_port()',
    'Ok(Json(product))',
    'update route',
  ],
]) {
  requireText(block, permission, `${label} permission`);
  requireText(block, operation, `${label} operation`);
  requireText(block, productIdentity, `${label} product identity`);
  requireText(block, 'headers: HeaderMap,', `${label} caller idempotency header`);
  requireText(block, 'admin_product_command_idempotency_key(&headers)?;', `${label} caller-owned identity`);
  requireText(block, 'admin_product_command_context(', `${label} resource-scoped context`);
  requireText(block, serviceCall, `${label} service contract`);
  requireText(block, 'map_admin_product_port_error(', `${label} owner-port mapper`);
  requireText(block, response, `${label} response contract`);
}

for (const [value, label] of [
  ['AdminProductErrorContext, ListProductsParams, ProductListItem,', 'shared product context imports'],
  ['admin_product_command_context, admin_product_command_idempotency_key,', 'shared Product command helpers'],
  ['map_admin_product_port_error,', 'shared Product owner-port mapper'],
  ['.list_admin_products(', 'mounted owner list read'],
  ['.read_product_projection(', 'mounted owner detail read'],
  ...['delete', 'publish', 'unpublish', 'archive'].map(name => [
    `super::super::products::${name}_product(state, tenant, auth, request_context, headers, path)`,
    `${name} delegation with authenticated request and retry identity`,
  ]),
]) requireText(adminProducts, value, label);

const sharedMapperUses =
  products.match(/map_admin_product_error\(\s+AdminProductErrorContext::new\(/g) ?? [];
if (sharedMapperUses.length !== 5) {
  failures.push(
    `expected five context-aware shared read mapper callsites, found ${sharedMapperUses.length}`,
  );
}
const wrapperMapperUses =
  adminProducts.match(/map_admin_product_port_error\(\s+AdminProductErrorContext::new\(/g) ?? [];
if (wrapperMapperUses.length !== 4) {
  failures.push(
    `expected four mounted Product owner-port mapper callsites, found ${wrapperMapperUses.length}`,
  );
}
const shippingValidationUses =
  adminProducts.match(
    /validate_admin_product_shipping_profile_input\(\s+runtime\.db\(\),\s+AdminProductShippingProfileErrorContext::new\(/g,
  ) ?? [];
if (shippingValidationUses.length !== 0) {
  failures.push(
    `expected no mutable shipping preflight before Product receipts, found ${shippingValidationUses.length}`,
  );
}

for (const [content, label] of [
  [products, 'shared product controller'],
  [adminProducts, 'admin product wrapper'],
]) {
  for (const value of [
    'fn map_product_service_error(',
    'fn map_product_database_error(',
    'fn map_product_write_error(',
    'super::admin_public_error(',
    'super::validate_product_shipping_profile_input(',
    'err.to_string()',
    'error.to_string()',
    'other.to_string()',
  ]) forbidText(content, value, `${label} stale or unsafe mapper`);
}

// Check actual Product-owner errors, not removed Commerce umbrella variants.
for (const tag of ['ImageNotFound', 'CannotDeleteOnlyVariant']) {
  requireText(policy, `CommerceError::${tag}`, 'Product owner exhaustive policy');
}
const scope = between(products, 'pub(crate) fn ensure_admin_product_scope(',
  'pub(crate) fn admin_product_command_context(', 'authenticated scope');
for (const marker of ['auth.tenant_id != tenant.id', 'request.tenant_id != tenant.id',
  'request.user_id != Some(auth.user_id)', 'HttpError::forbidden(']) requireText(scope, marker, 'scope enforcement');
for (const [content,names] of [[products,['list_products','show_product','delete_product','publish_product','unpublish_product','archive_product']],
  [adminProducts,['list_products','show_product','create_product','update_product']]]) {
  for (const name of names) {
    const start = content.indexOf(`pub async fn ${name}(`);
    const end = content.indexOf('pub async fn ', start + 1);
    const route = content.slice(start, end < 0 ? undefined : end);
    requireText(route, 'ensure_admin_product_scope(&tenant, &auth, &request_context)?;', `${name} authenticated scope`);
    const admission = route.indexOf('ensure_admin_product_scope(');
    for (const io of ['.product_catalog_command_port()', '.product_catalog_read_port()', 'CatalogService::new(', 'product::Entity::find()']) {
      if(route.includes(io) && route.indexOf(io) < admission) failures.push(`${name}: ${io} must follow scope admission`);
    }
  }
}
if (failures.length > 0) {
  console.error('Commerce admin product route error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin product routes retain typed causes and truthful route context',
);
