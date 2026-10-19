#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => {
  const fileUrl = new URL(relativePath, root);
  if (existsSync(fileUrl)) return readFileSync(fileUrl, 'utf8');
  const dirRelative = relativePath.endsWith('.rs') ? relativePath.slice(0, -3) : relativePath;
  const dirUrl = new URL(dirRelative, root);
  if (existsSync(dirUrl)) {
    return readdirSync(dirUrl)
      .filter((file) => file.endsWith('.rs'))
      .map((file) => readFileSync(new URL(`${dirRelative}/${file}`, root), 'utf8'))
      .join('\n');
  }
  return readFileSync(fileUrl, 'utf8');
};

const router = read('crates/modules/rustok-commerce/src/controllers/store/mod.rs');
const mountedList = read('crates/modules/rustok-commerce/src/controllers/store/products_owner_list.rs');
const legacyProducts = read('crates/modules/rustok-commerce/src/controllers/store/products.rs');
const ownerHttp = read('crates/modules/rustok-product/src/storefront_http_read_port.rs');
const productPorts = read('crates/modules/rustok-product/src/ports.rs');
const productQueries = read('crates/modules/rustok-product/src/services/catalog/queries.rs');
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

const listStart = mountedList.indexOf('pub async fn list_products(');
const listHandler = listStart < 0 ? '' : mountedList.slice(listStart);
if (listStart < 0) failures.push('mounted storefront Product list: unable to isolate source block');
const listMapper = between(
  mountedList,
  'fn map_storefront_product_list_port_error(',
  '/// List published storefront products through the Product-owned legacy HTTP projection.',
  'mounted storefront Product list mapper',
);
const detailHandler = between(
  legacyProducts,
  'pub async fn show_product(',
  '/// List available storefront regions',
  'mounted storefront Product detail implementation',
);
const detailMapper = between(
  legacyProducts,
  'fn map_storefront_product_port_error(',
  'fn map_storefront_auxiliary_port_error(',
  'storefront Product detail mapper',
);

for (const [value, label] of [
  ['#[path = "products.rs"]\npub mod products_legacy;', 'legacy Product source alias'],
  ['#[path = "products_owner_list.rs"]\npub mod products;', 'mounted Product module alias'],
  ['.route("/products", axum::routing::get(products::list_products))', 'mounted list route'],
  ['.route("/products/{id}", axum::routing::get(products::show_product))', 'mounted detail route'],
]) requireText(router, value, label);

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;', 'list channel guard'],
  ['.product_storefront_http_read_port()', 'host-selected optional list owner capability'],
  ['.list_legacy_storefront_http_products(', 'list owner call'],
  ['LegacyStorefrontHttpProductsRequest {', 'typed list request'],
  ['let page = pagination.page.max(1);', 'list page normalization'],
  ['let per_page = pagination.limit();', 'list page-size clamp'],
  ['fallback_locale: Some(tenant.default_locale.clone())', 'list fallback locale'],
  ['shipping_profile_slug: Some(item.shipping_profile_slug)', 'list response projection'],
  ['PaginationMeta::new(list.page, list.per_page, list.total)', 'list pagination response'],
]) requireText(listHandler, value, label);

for (const value of [
  'CatalogService::new(',
  'product::Entity::find()',
  'product_translation::Entity::find()',
  '.load_product_tag_map(',
  'product_translation_title_search_condition(',
]) forbidText(listHandler, value, 'mounted list foreign Product storage/service access');

const portPolicy = between(legacyProducts,
  'pub(super) fn storefront_product_port_http_policy(',
  'fn map_storefront_product_port_error(', 'shared Product port HTTP policy');
for (const marker of [
  'PortErrorKind::Validation', 'PortErrorKind::NotFound',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout', 'PortErrorKind::InvariantViolation',
  'PortErrorKind::Conflict', 'PortErrorKind::Forbidden',
  'StatusCode::CONFLICT', 'StatusCode::FORBIDDEN',
  '"commerce_store_product_invalid"', '"commerce_store_not_found"',
  '"commerce_store_product_unavailable"', '"commerce_store_product_failed"',
  '"commerce_store_product_conflict"', '"commerce_product_access_denied"',
]) requireText(portPolicy, marker, 'shared static Product HTTP outcome');
for (const [kind, status, code] of [
  ['Conflict', 'CONFLICT', 'commerce_store_product_conflict'],
  ['Forbidden', 'FORBIDDEN', 'commerce_product_access_denied'],
]) requireText(portPolicy.replace(/\s/g, ''), `PortErrorKind::${kind}=>(StatusCode::${status},"${code}",`, 'typed refusal must retain its correct HTTP status/code');
forbidText(portPolicy, 'PortErrorKind::Conflict | PortErrorKind::Forbidden', 'domain refusal must not collapse to invariant');
forbidText(portPolicy, '_ =>', 'HTTP policy must remain exhaustive');
for (const [mapper, prefix] of [[listMapper, 'list'], [detailMapper, 'detail']]) {
  for (const marker of [
    'storefront_product_port_http_policy(&error.kind)',
    'owner_error_kind = ?error.kind', 'owner_code_length = error.code.chars().count()',
    'retryable = error.retryable', 'correlation_id_length = context.correlation_id.chars().count()',
    'HttpError::new(status, code, message)',
  ]) requireText(mapper, marker, `${prefix}: shared policy and bounded diagnostics`);
  for (const value of ['match &error.kind', 'error = ?error', 'error.message', 'error.to_string()',
    'err.to_string()', 'correlation_id = %context.correlation_id']) {
    forbidText(mapper, value, `${prefix}: raw payload or parallel classifier`);
  }
}
const listContext = between(mountedList, 'fn storefront_product_list_port_context(', 'fn storefront_product_list_unavailable(', 'list context factory');
const detailContext = between(legacyProducts, 'fn storefront_product_port_context(', 'pub(super) fn storefront_product_port_http_policy(', 'detail context factory');
for (const [source, label] of [[listContext, 'list'], [detailContext, 'detail']]) {
  requireText(source, 'Uuid::new_v4().to_string()', `${label}: fresh opaque owner correlation`);
  forbidText(source, 'format!(', `${label}: do not encode page/resource identity in correlation`);
}
forbidText(detailHandler, 'tenant_id = %tenant.id', 'bounded mounted detail diagnostic');
forbidText(detailHandler, 'product_id = %id', 'bounded mounted detail diagnostic');

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;', 'detail channel guard'],
  ['runtime\n        .product_catalog_read_port()', 'detail host-selected Product read port'],
  ['.read_storefront_product_projection(', 'detail owner read'],
  ['StorefrontProductProjectionSubject::ProductId { product_id: id }', 'detail Product identity'],
  ['locale: Some(request_context.locale.clone())', 'detail requested locale'],
  ['fallback_locale: Some(tenant.default_locale.clone())', 'detail fallback locale'],
  ['"commerce_store_not_found"', 'hidden detail not-found envelope'],
]) requireText(detailHandler, value, label);
for (const value of [
  'CatalogService::new(',
  '.get_product_with_locale_fallback(',
  'product.status != product::ProductStatus::Active',
  'apply_public_channel_inventory_to_product(',
]) forbidText(detailHandler, value, 'stale concrete Product detail path');

for (const [value, label] of [
  ['pub trait ProductStorefrontHttpReadPort', 'Product HTTP owner capability'],
  ['MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE: u64 = 100', 'REST page-size compatibility'],
  ['product::Column::Status.eq(product::ProductStatus::Active)', 'owner active filter'],
  ['product::Column::PublishedAt.is_not_null()', 'owner published filter'],
  ['rustok_inventory::is_metadata_visible_for_public_channel(', 'owner channel visibility'],
  ['let total = visible_products.len() as u64;', 'owner visible total'],
  ['usize::try_from(offset)', 'owner checked pagination offset conversion'],
  ['.skip(offset_usize)', 'owner pagination offset'],
  ['.take(per_page as usize)', 'owner pagination limit'],
  ['.unwrap_or_default()', 'owner empty missing translation'],
  ['shipping_profile_slug_from_metadata(&product.metadata)', 'owner metadata-only shipping profile'],
  ['.load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))', 'owner tag projection'],
]) requireText(ownerHttp, value, label);

for (const [value, label] of [
  ['async fn read_storefront_product_projection(', 'Product detail owner capability'],
  ['StorefrontProductProjectionSubject::ProductId { product_id }', 'Product detail owner subject'],
  ['get_published_product_by_id_with_locale_fallback(', 'Product published detail implementation'],
]) requireText(productPorts, value, label);
for (const [value, label] of [
  ['product.status != entities::product::ProductStatus::Active', 'detail active visibility'],
  ['product.published_at.is_none()', 'detail publication visibility'],
  ['is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)', 'detail channel visibility'],
  ['apply_public_channel_inventory_to_product(', 'detail public inventory'],
]) requireText(productQueries, value, label);

for (const [value, label] of [
  ['super::products_legacy::show_product(', 'detail wrapper delegation'],
  ['super::products_legacy::list_regions(', 'region wrapper delegation'],
  ['super::products_legacy::list_shipping_options(', 'shipping-options wrapper delegation'],
]) requireText(mountedList, value, label);
requireText(legacyProducts, 'pub async fn list_regions(', 'legacy region handler retained');
requireText(
  legacyProducts,
  'pub async fn list_shipping_options(',
  'legacy shipping-options handler retained',
);

if (failures.length > 0) {
  console.error('Commerce storefront Product HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted storefront Product list/detail use shared typed HTTP outcomes and bounded owner diagnostics');
