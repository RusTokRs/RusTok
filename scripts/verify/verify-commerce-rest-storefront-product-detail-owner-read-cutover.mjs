#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/products.rs');
const storeRouter = read('crates/rustok-commerce/src/controllers/store/mod.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const productPorts = read('crates/rustok-product/src/ports.rs');
const productQueries = read('crates/rustok-product/src/services/catalog/queries.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-product-detail-owner-read-cutover-2026-08-09.md',
);
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

const detail = between(
  controller,
  'pub async fn show_product(',
  '/// List available storefront regions',
  'mounted storefront product detail',
);
const ownerMapper = between(
  controller,
  'fn map_storefront_product_port_error(',
  'fn map_storefront_auxiliary_port_error(',
  'storefront Product owner mapper',
);
const listHandler = between(
  controller,
  'pub async fn list_products(',
  '/// Show published storefront product',
  'storefront Product list compatibility path',
);

for (const [value, label] of [
  ['.route("/products/{id}", axum::routing::get(products::show_product))', 'mounted detail route'],
  ['pub mod products;', 'mounted products module'],
]) requireText(storeRouter, value, label);

for (const [value, label] of [
  ['runtime\n        .product_catalog_read_port()', 'host-selected Product read port'],
  ['.read_storefront_product_projection(', 'owner storefront detail call'],
  ['StorefrontProductProjectionRequest {', 'typed owner detail request'],
  ['StorefrontProductProjectionSubject::ProductId { product_id: id }', 'typed Product subject'],
  ['locale: Some(request_context.locale.clone())', 'requested locale forwarding'],
  ['fallback_locale: Some(tenant.default_locale.clone())', 'tenant locale forwarding'],
  ['public_channel_slug,', 'public channel forwarding'],
  ['storefront_product_port_context(', 'bounded Product call context'],
  ['"read_storefront_product_projection"', 'owner operation identity'],
  ['HttpError::not_found("commerce_store_not_found", "Commerce resource not found")', 'hidden/missing public envelope'],
]) requireText(detail, value, label);

for (const value of [
  'CatalogService::new(',
  '.get_product_with_locale_fallback(',
  'product.status != product::ProductStatus::Active',
  'apply_public_channel_inventory_to_product(',
  'show_product_inventory',
]) forbidText(detail, value, 'stale concrete Product detail path');

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable mapping'],
  ['PortErrorKind::Conflict | PortErrorKind::Forbidden | PortErrorKind::InvariantViolation', 'fail-closed mapping'],
  ['"commerce_store_product_invalid"', 'validation code'],
  ['"commerce_store_not_found"', 'not-found code'],
  ['"commerce_store_product_unavailable"', 'unavailable code'],
  ['"commerce_store_product_failed"', 'fail-closed code'],
  ['owner = "rustok_product"', 'owner identity diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['owner_error_kind = ?error.kind', 'owner error kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'retryable diagnostic'],
]) requireText(ownerMapper, value, label);

for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'err.to_string()']) {
  forbidText(ownerMapper, value, 'raw Product owner diagnostic');
}

for (const [value, label] of [
  ['fn product_catalog_read_port(', 'HTTP Product read accessor'],
  ['self.product_catalog_read_runtime.read_port()', 'HTTP Product read projection'],
  ['shared_get::<rustok_product::ProductCatalogReadRuntime>()', 'host-selected Product runtime'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['pub trait ProductCatalogReadPort', 'Product owner read port'],
  ['async fn read_storefront_product_projection(', 'Product owner storefront detail capability'],
  ['context\n            .require_policy(PortCallPolicy::read())', 'Product read admission'],
  ['StorefrontProductProjectionSubject::ProductId { product_id }', 'Product owner product-id dispatch'],
  ['get_published_product_by_id_with_locale_fallback(', 'Product owner published detail implementation'],
]) requireText(productPorts, value, label);

for (const [value, label] of [
  ['product.status != entities::product::ProductStatus::Active', 'owner active visibility'],
  ['product.published_at.is_none()', 'owner published visibility'],
  ['is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)', 'owner channel visibility'],
  ['apply_public_channel_inventory_to_product(', 'owner public inventory projection'],
]) requireText(productQueries, value, label);

for (const [value, label] of [
  ['CatalogService::new(runtime.db_clone(), runtime.event_bus())', 'legacy list concrete Product service remains explicit'],
  ['product_translation_title_search_condition(', 'legacy REST localized search remains explicit'],
]) requireText(listHandler, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology item remains open',
);

for (const [value, label] of [
  ['# Commerce REST storefront Product detail owner-read cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['`ProductCatalogReadPort::read_storefront_product_projection`', 'record owner operation'],
  ['The storefront Product list remains outside this slice', 'record explicit list exclusion'],
  ['The canonical ecommerce topology item remains open', 'record broad P0 open'],
  ['no tests, Cargo commands, Node verifiers, formatter', 'record no validation execution'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST storefront Product detail owner-read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted REST storefront Product detail uses the host-selected Product owner read port');
