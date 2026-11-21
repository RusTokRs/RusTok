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
const commerceErrors = read('crates/modules/rustok-commerce-foundation/src/error.rs');
const catalogService = read('crates/modules/rustok-product/src/services/catalog.rs');
const catalogCommands = read('crates/modules/rustok-product/src/services/catalog/commands.rs');
const catalogProjection = read('crates/modules/rustok-product/src/services/catalog/projection.rs');
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

const mapper = between(
  products,
  'pub(crate) fn map_admin_product_port_error(',
  '/// Shared admin product delete handler.',
  'shared admin product mapper',
);
const deleteHandler = between(
  products,
  'pub async fn delete_product(',
  '/// Shared admin product publish handler.',
  'product delete handler',
);
const publishHandler = between(
  products,
  'pub async fn publish_product(',
  '/// Shared admin product unpublish handler.',
  'product publish handler',
);
const unpublishHandler = between(
  products,
  'pub async fn unpublish_product(',
  '#[derive(Debug, serde::Deserialize, ToSchema, utoipa::IntoParams)]',
  'product unpublish handler',
);

for (const [value, label] of [
  ['const ADMIN_PRODUCT_OWNER: &str = "rustok_product.catalog";', 'product owner identity'],
  ['owner = ADMIN_PRODUCT_OWNER', 'product owner logging'],
  ['const ADMIN_PRODUCT_BOUNDARY: &str = "commerce_admin_product_http";', 'product HTTP boundary identity'],
  ['boundary = ADMIN_PRODUCT_BOUNDARY', 'product HTTP boundary logging'],
  ['error_kind,', 'error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['owner_code_length', 'bounded owner-code diagnostics'],
  ['fn product_port_http_policy(', 'shared port error policy'],
  ['"unexpected_owner_error"', 'unexpected owner kind'],
  ['HttpError::new(status, code, message)', 'static envelope construction'],
]) {
  requireText(products, value, label);
}

// The owner crosses the port boundary as a typed PortError; the HTTP mapper
// dispatches on the kind plus the stable owner code strings.
for (const [value, label] of [
  ['match &error.kind', 'typed port error dispatch'],
  ['PortErrorKind::Validation if error.code == "product.revision_required"', 'revision guard variant'],
  ['PortErrorKind::Validation', 'validation variant'],
  ['PortErrorKind::NotFound', 'not-found variant'],
  ['PortErrorKind::Conflict if error.code == "product.duplicate_handle"', 'duplicate handle variant'],
  ['PortErrorKind::Conflict if error.code == "product.duplicate_sku"', 'duplicate SKU variant'],
  ['PortErrorKind::Conflict if error.code == "product.revision_conflict"', 'revision conflict variant'],
  ['PortErrorKind::Conflict', 'state conflict variant'],
  ['PortErrorKind::Forbidden', 'forbidden variant'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'storage unavailable variant'],
  ['PortErrorKind::InvariantViolation', 'fail-closed variant'],
  ['inventory_deletion_public_failure(&error)', 'inventory deletion failure delegation'],
  ['inventory_quantity_public_failure(&error)', 'inventory quantity failure delegation'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_product_handle_conflict"', 'handle conflict code'],
  ['"commerce_admin_product_sku_conflict"', 'SKU conflict code'],
  ['"commerce_admin_product_invalid"', 'product invalid code'],
  ['"commerce_admin_product_state_conflict"', 'state conflict code'],
  ['"commerce_admin_product_storage_unavailable"', 'storage unavailable code'],
  ['"commerce_admin_product_failed"', 'fail-closed code'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'other.to_string()',
  'commerce_operation_failed',
]) {
  forbidText(products, value, 'unsafe shared admin product public conversion');
}
forbidText(mapper, 'HttpError::bad_request(', 'mapper must construct the envelope statically');

const portMapperUses = products.match(/map_admin_product_port_error/g) ?? [];
if (portMapperUses.length !== 7) {
  failures.push(
    `expected port mapper definition plus six handler callsites, found ${portMapperUses.length}`,
  );
}

for (const [content, values, label] of [
  [
    deleteHandler,
    [
      'Permission::PRODUCTS_DELETE',
      '.product_catalog_command_port()',
      '.delete_product(port_context.clone(), id)',
      'map_admin_product_port_error(',
      'Ok(StatusCode::NO_CONTENT)',
    ],
    'product delete path',
  ],
  [
    publishHandler,
    [
      'Permission::PRODUCTS_UPDATE',
      '.product_catalog_command_port()',
      '.publish_product(port_context.clone(), id)',
      'map_admin_product_port_error(',
      'Ok(Json(product))',
    ],
    'product publish path',
  ],
  [
    unpublishHandler,
    [
      'Permission::PRODUCTS_UPDATE',
      '.product_catalog_command_port()',
      '.unpublish_product(port_context.clone(), id)',
      'map_admin_product_port_error(',
      'Ok(Json(product))',
    ],
    'product unpublish path',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

const sharedMapperUses = products.match(/map_admin_product_port_error/g) ?? [];
if (sharedMapperUses.length !== 7) {
  failures.push(
    `expected shared port mapper definition plus six callsites, found ${sharedMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['pub async fn list_products(', 'admin list handler'],
  ['pub async fn show_product(', 'admin detail handler'],
  ['super::super::products::delete_product(', 'admin delete delegation'],
  ['super::super::products::publish_product(', 'admin publish delegation'],
  ['super::super::products::unpublish_product(', 'admin unpublish delegation'],
  ['map_admin_product_port_error,', 'shared port mapper import'],
]) {
  requireText(adminProducts, value, label);
}

for (const [value, label] of [
  ['Database(#[from] sea_orm::DbErr)', 'owner database variant'],
  ['ProductNotFound(Uuid)', 'owner product variant'],
  ['VariantNotFound(Uuid)', 'owner variant variant'],
  ['DuplicateHandle { handle: String, locale: String }', 'owner handle variant'],
  ['DuplicateSku(String)', 'owner SKU variant'],
  ['InvalidPrice(String)', 'owner price variant'],
  ['InsufficientInventory { requested: i32, available: i32 }', 'owner inventory variant'],
  ['InvalidOptionCombination', 'owner option variant'],
  ['Validation(String)', 'owner validation variant'],
  ['ShippingProfileNotFound(Uuid)', 'owner shipping-profile variant'],
  ['DuplicateShippingProfileSlug(String)', 'owner profile-conflict variant'],
  ['NoVariants', 'owner no-variants variant'],
  ['CannotDeletePublished', 'owner state variant'],
  ['Rich(#[source] Box<RichError>)', 'owner rich variant'],
  ['Core(#[from] CoreError)', 'owner core variant'],
]) {
  requireText(commerceErrors, value, label);
}

for (const [value, label] of [
  ['pub async fn get_product_with_locale_fallback(', 'catalog detail operation'],
]) {
  requireText(catalogProjection, value, label);
}
for (const [value, label] of [
  ['pub async fn delete_product(', 'catalog delete operation'],
  ['pub async fn publish_product(', 'catalog publish operation'],
  ['pub async fn unpublish_product(', 'catalog unpublish operation'],
  ['CommerceError::ProductNotFound(product_id)', 'catalog not-found construction'],
  ['CommerceError::CannotDeletePublished', 'catalog state-conflict construction'],
  ['-> CommerceResult<ProductResponse>', 'typed product response result'],
]) {
  requireText(catalogCommands, value, label);
}

if (failures.length > 0) {
  console.error('Commerce admin product read/action HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Admin product list/detail/delete/publish errors use stable typed public envelopes');
