#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const router = read('crates/rustok-commerce/src/controllers/store/mod.rs');
const mounted = read('crates/rustok-commerce/src/controllers/store/products_owner_list.rs');
const legacy = read('crates/rustok-commerce/src/controllers/store/products.rs');
const owner = read('crates/rustok-product/src/storefront_http_read_port.rs');
const runtime = read('crates/rustok-product/src/runtime.rs');
const lib = read('crates/rustok-product/src/lib.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-product-list-owner-read-cutover-2026-08-10.md',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "products.rs"]\npub mod products_legacy;', 'legacy Product compatibility module'],
  ['#[path = "products_owner_list.rs"]\npub mod products;', 'mounted Product owner-list module'],
  ['.route("/products", axum::routing::get(products::list_products))', 'mounted list route'],
  ['.route("/products/{id}", axum::routing::get(products::show_product))', 'preserved detail route'],
]) requireText(router, value, label);

for (const [value, label] of [
  ['ProductStorefrontHttpReadPort', 'Product HTTP owner port type'],
  ['self.product_catalog_read_runtime.storefront_http_read_port()', 'host-selected optional capability'],
  ['ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;', 'channel guard'],
  ['let page = pagination.page.max(1);', 'page normalization'],
  ['let per_page = pagination.limit();', '100-row REST clamp'],
  ['unwrap_or(request_context.locale.as_str())', 'effective locale'],
  ['public_channel_slug_from_request(&request_context)', 'public channel'],
  ['PortActor::service("rustok-commerce.storefront-product")', 'service actor'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded deadline'],
  ['.product_storefront_http_read_port()', 'runtime capability lookup'],
  ['.ok_or_else(storefront_product_list_unavailable)?', 'missing capability fail closed'],
  ['.list_legacy_storefront_http_products(', 'owner list call'],
  ['LegacyStorefrontHttpProductsRequest {', 'typed owner request'],
  ['locale: Some(locale.to_string())', 'locale forwarding'],
  ['fallback_locale: Some(tenant.default_locale.clone())', 'tenant fallback forwarding'],
  ['vendor: params.vendor', 'vendor forwarding'],
  ['product_type: params.product_type', 'product-type forwarding'],
  ['search: params.search', 'search forwarding'],
  ['page,', 'page forwarding'],
  ['per_page,', 'page-size forwarding'],
  ['shipping_profile_slug: Some(item.shipping_profile_slug)', 'shipping-profile projection'],
  ['PaginationMeta::new(list.page, list.per_page, list.total)', 'pagination metadata'],
  ['path = "/store/products/{id}"', 'detail OpenAPI wrapper'],
  ['super::products_legacy::show_product(', 'detail wrapper delegation'],
  ['path = "/store/regions"', 'regions OpenAPI wrapper'],
  ['super::products_legacy::list_regions(', 'regions wrapper delegation'],
  ['path = "/store/shipping-options"', 'shipping-options OpenAPI wrapper'],
  ['super::products_legacy::list_shipping_options(', 'shipping-options wrapper delegation'],
]) requireText(mounted, value, label);

for (const value of [
  'CatalogService::new(',
  'product::Entity::find()',
  'product_translation::Entity::find()',
  'product_translation_title_search_condition(',
  'load_product_tag_map(',
  'is_metadata_visible_for_public_channel(',
  'error = ?error',
  'error.message',
  'error.to_string()',
]) forbidText(mounted, value, 'mounted Product module must not own Product storage/backend details');

for (const [value, label] of [
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_store_product_invalid"', 'validation code'],
  ['"commerce_store_not_found"', 'not-found code'],
  ['"commerce_store_product_unavailable"', 'unavailable code'],
  ['"commerce_store_product_failed"', 'fail-closed code'],
  ['owner_error_kind = ?error.kind', 'bounded error-kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner-code diagnostic'],
  ['retryable = error.retryable', 'bounded retryable diagnostic'],
]) requireText(mounted, value, label);

for (const [value, label] of [
  ['pub trait ProductStorefrontHttpReadPort', 'Product owner HTTP port'],
  ['async fn list_legacy_storefront_http_products(', 'Product owner HTTP operation'],
  ['MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE: u64 = 100', 'REST page-size contract'],
  ['context.require_policy(PortCallPolicy::read())?', 'read admission'],
  ['product::Column::TenantId.eq(tenant_id)', 'tenant filter'],
  ['product::Column::Status.eq(product::ProductStatus::Active)', 'active filter'],
  ['product::Column::PublishedAt.is_not_null()', 'published filter'],
  ['product::Column::Vendor.eq(vendor)', 'vendor filter'],
  ['product::Column::ProductType.eq(product_type)', 'product-type filter'],
  ['product_title_search_condition(', 'raw title search'],
  ['.order_by_desc(product::Column::PublishedAt)', 'published ordering'],
  ['.order_by_desc(product::Column::CreatedAt)', 'created ordering'],
  ['.all(db)', 'pre-visibility materialization'],
  ['rustok_inventory::is_metadata_visible_for_public_channel(', 'owner channel visibility'],
  ['let total = visible_products.len() as u64;', 'post-visibility total'],
  ['.skip(offset as usize)', 'post-visibility page offset'],
  ['.take(per_page as usize)', 'post-visibility page size'],
  ['pick_product_translation(items.as_slice(), locale, fallback_locale)', 'locale fallback projection'],
  ['.unwrap_or_default()', 'empty missing translation projection'],
  ['shipping_profile_slug_from_metadata(&product.metadata)', 'metadata-only shipping profile'],
  ['.load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))', 'owner tag projection'],
]) requireText(owner, value, label);

const visibilityIndex = owner.indexOf('rustok_inventory::is_metadata_visible_for_public_channel(');
const skipIndex = owner.indexOf('.skip(offset as usize)');
if (visibilityIndex < 0 || skipIndex < 0 || visibilityIndex > skipIndex) {
  failures.push('owner REST compatibility projection must apply channel visibility before pagination');
}

for (const [value, label] of [
  ['storefront_http_read_port: Option<Arc<dyn ProductStorefrontHttpReadPort>>', 'optional runtime capability'],
  ['storefront_http_read_port: None', 'external-safe default'],
  ['.with_storefront_http_read_port(catalog.clone())', 'embedded owner capability'],
  ['pub fn with_storefront_http_read_port(', 'explicit external capability builder'],
  ['pub fn storefront_http_read_port(&self)', 'runtime capability accessor'],
  ['pub fn external(read_port: Arc<dyn ProductCatalogReadPort>)', 'external runtime constructor'],
]) requireText(runtime, value, label);
requireText(lib, 'mod storefront_http_read_port;', 'Product owner module registration');
requireText(lib, 'ProductStorefrontHttpReadPort', 'Product HTTP owner port export');
requireText(lib, 'LegacyStorefrontHttpProductsRequest', 'Product HTTP request export');

const externalStart = runtime.indexOf('pub fn external(read_port: Arc<dyn ProductCatalogReadPort>)');
const httpBuilderStart = runtime.indexOf('pub fn with_storefront_http_read_port(');
if (externalStart < 0 || httpBuilderStart < 0 || externalStart > httpBuilderStart) {
  failures.push('unable to verify external Product runtime fail-closed capability composition');
} else {
  forbidText(
    runtime.slice(externalStart, httpBuilderStart),
    'with_storefront_http_read_port',
    'external Product runtime must not silently install embedded HTTP capability',
  );
}

for (const [value, label] of [
  ['CatalogService::new(runtime.db_clone(), runtime.event_bus())', 'legacy concrete list compatibility source'],
  ['product_translation_title_search_condition(', 'legacy raw search compatibility source'],
]) requireText(legacy, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);
for (const [value, label] of [
  ['# Commerce REST storefront Product list owner-read cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['ProductStorefrontHttpReadPort::list_legacy_storefront_http_products', 'record owner operation'],
  ['does not currently restrict the search by locale', 'record raw search parity'],
  ['metadata-only shipping-profile', 'record shipping-profile parity'],
  ['thin annotated wrappers', 'record wrapper topology'],
  ['The canonical ecommerce P0 item', 'record broad P0 open'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record no validation execution'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST storefront Product list owner-read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted REST storefront Product list uses the host-selected Product owner HTTP capability');
