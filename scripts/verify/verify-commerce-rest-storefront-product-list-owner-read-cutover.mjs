#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const router = read('crates/modules/rustok-commerce/src/controllers/store/mod.rs');
const mounted = read('crates/modules/rustok-commerce/src/controllers/store/products.rs');
const owner = read('crates/modules/rustok-product/src/storefront_http_read_port.rs');
const ports = read('crates/modules/rustok-product/ports/src/ports.rs');
const runtime = read('crates/modules/rustok-product/src/runtime.rs');
const commerceRuntime = read('crates/modules/rustok-commerce/src/controllers/mod.rs');
const lib = read('crates/modules/rustok-product/src/lib.rs');
const plan = read('crates/modules/rustok-commerce/docs/implementation-plan.md');

const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['pub mod products;', 'mounted Product controller'],
  ['.route("/products", axum::routing::get(products::list_products))', 'mounted list route'],
  ['.route("/products/{id}", axum::routing::get(products::show_product))', 'preserved detail route'],
]) requireText(router, value, label);

for (const [value, label] of [
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
  ['path = "/store/regions"', 'regions OpenAPI wrapper'],
  ['path = "/store/shipping-options"', 'shipping-options OpenAPI wrapper'],
]) requireText(mounted, value, label);

for (const value of [
  'CatalogService::new(',
  'entities::{product',
  'product::Entity::find()',
  'product_translation::Entity::find()',
  'product_translation_title_search_condition(',
  'load_product_tag_map(',
  'error = ?error',
  'error.message',
  'error.to_string()',
]) forbidText(mounted, value, 'mounted Product module must not own Product storage/backend details');

const policyStart = mounted.indexOf('pub(super) fn storefront_product_port_http_policy(');
const policyEnd = mounted.indexOf('fn map_storefront_product_port_error(', policyStart);
if (policyStart < 0 || policyEnd < 0) failures.push('shared Product HTTP policy is missing');
const sharedPolicy = mounted.slice(policyStart, policyEnd);
const listStart = mounted.indexOf('pub async fn list_products(');
const listEnd = mounted.indexOf('/// Show published storefront product', listStart);
if (listStart < 0 || listEnd < 0) failures.push('unable to isolate mounted list handler');
requireText(mounted.slice(listStart, listEnd), 'map_storefront_product_port_error(', 'mounted list delegates the shared mapper');
forbidText(mounted.slice(listStart, listEnd), 'is_metadata_visible_for_public_channel(', 'Product owner applies list visibility');
requireText(mounted, 'storefront_product_port_http_policy(&error.kind)', 'shared mapper delegates public policy');
for (const [value, label] of [
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_store_product_invalid"', 'validation code'],
  ['"commerce_store_not_found"', 'not-found code'],
  ['"commerce_store_product_unavailable"', 'unavailable code'],
  ['"commerce_store_product_failed"', 'fail-closed code'],
  ['StatusCode::FORBIDDEN', 'permission status'],
  ['StatusCode::CONFLICT', 'conflict status'],
]) requireText(sharedPolicy, value, label);
for (const [value, label] of [
  ['owner_error_kind = ?error.kind', 'bounded error-kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner-code diagnostic'],
  ['retryable = error.retryable', 'bounded retryable diagnostic'],
]) requireText(mounted, value, label);

for (const [value, label] of [
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
  ['product_channel_visibility_condition(', 'owner SQL channel visibility'],
  ['let total = query.clone().count(db).await?;', 'count the same filtered query'],
  ['.order_by_desc(product::Column::Id)', 'deterministic page ordering'],
  ['.offset(offset)', 'database page offset'],
  ['.limit(per_page)', 'database page limit'],
  ['pick_product_translation(items.as_slice(), locale, fallback_locale)', 'locale fallback projection'],
  ['.unwrap_or_default()', 'empty missing translation projection'],
  ['shipping_profile_slug_from_metadata(&product.metadata)', 'metadata-only shipping profile'],
  ['Some(context),', 'owner tag projection'],
]) requireText(owner, value, label);

requireText(ports, 'pub trait ProductStorefrontHttpReadPort', 'published Product owner HTTP port');
const queryStart = owner.indexOf('fn legacy_storefront_http_products_query(');
const projection = owner.indexOf('\nasync fn list_legacy_storefront_http_products(', queryStart);
requireText(owner.slice(queryStart, projection), 'product_channel_visibility_condition(', 'visibility is part of the owner SQL query');
const ownerList = owner.slice(projection);
const filteredQuery = ownerList.indexOf('let query = legacy_storefront_http_products_query(');
const count = ownerList.indexOf('let total = query.clone().count(db).await?;');
const page = ownerList.indexOf('.offset(offset)');
if (queryStart < 0 || projection < 0 || filteredQuery < 0 || count <= filteredQuery || page <= count) {
  failures.push('owner REST projection must count and page the same visible SQL query');
}

for (const [value, label] of [
  ['fn product_storefront_http_read_port(', 'Commerce host Product HTTP owner accessor'],
  ['Option<std::sync::Arc<dyn rustok_product::ProductStorefrontHttpReadPort>>', 'typed optional Product HTTP capability'],
  ['self.product_catalog_read_runtime.storefront_http_read_port()', 'Commerce host delegates to Product runtime'],
]) requireText(commerceRuntime.replace(/\s/g, ''), value.replace(/\s/g, ''), label);

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

forbidText(router, 'mod products_legacy', 'obsolete Product controller');
if ((mounted.match(/pub async fn list_products\(/g) ?? []).length !== 1) {
  failures.push('storefront Product must declare one list handler');
}

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);


if (failures.length > 0) {
  console.error('Commerce REST storefront Product list owner-read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted REST storefront Product list uses the host-selected Product owner HTTP capability');
