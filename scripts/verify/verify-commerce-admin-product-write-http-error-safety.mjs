#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(path.resolve(configuredRoot) + path.sep)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const products = read('crates/modules/rustok-commerce/src/controllers/admin/products.rs');
const sharedProducts = read('crates/modules/rustok-commerce/src/controllers/products.rs');
const runtime = read('crates/modules/rustok-commerce/src/controllers/mod.rs');
const commandPort = read('crates/modules/rustok-product/src/catalog_command_port.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(label + ': missing ' + value);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(label + ': forbidden ' + value);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(label + ': unable to isolate source block');
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const createHandler = between(
  products,
  'pub async fn create_product(',
  '/// Show admin ecommerce product',
  'admin Product create handler',
);
const updateHandler = between(
  products,
  'pub async fn update_product(',
  '/// Delete admin ecommerce product',
  'admin Product update handler',
);
const sharedIdentity = between(
  sharedProducts,
  'pub(crate) fn admin_product_command_idempotency_key(',
  'pub(crate) fn admin_product_lifecycle_idempotency_key',
  'shared Product command identity helper',
);
const sharedContext = between(
  sharedProducts,
  'pub(crate) fn admin_product_command_context(',
  'pub(crate) fn map_admin_product_port_error(',
  'shared Product command context',
);
const portMapper = between(
  sharedProducts,
  'pub(crate) fn map_admin_product_port_error(',
  '/// Shared admin product list handler.',
  'active Product owner-port mapper',
);

for (const [source, values, label] of [
  [createHandler, [
    'headers: HeaderMap,',
    'Permission::PRODUCTS_CREATE',
    'admin_product_command_idempotency_key(&headers)?',
    'admin_product_command_context(',
    '.product_catalog_command_port()',
    '.create_product(port_context.clone(), input)',
    'map_admin_product_port_error(',
    'StatusCode::CREATED',
  ], 'Product create path'],
  [updateHandler, [
    'headers: HeaderMap,',
    'Permission::PRODUCTS_UPDATE',
    'admin_product_command_idempotency_key(&headers)?',
    'admin_product_command_context(',
    '.product_catalog_command_port()',
    '.update_product(port_context.clone(), id, input)',
    'map_admin_product_port_error(',
    'StatusCode::OK',
  ], 'Product update path'],
]) {
  for (const value of values) requireText(source, value, label);
  for (const value of [
    'admin_product_command_idempotency_key(tenant.id,',
    'admin_product_command_idempotency_key(tenant.id, auth.user_id',
    'CatalogService::new(',
  ]) forbidText(source, value, label);
}

for (const [value, label] of [
  ['path = "/admin/products"', 'create OpenAPI route'],
  ['"Idempotency-Key" = String, Header', 'create OpenAPI caller identity'],
  ['path = "/admin/products/{id}"', 'update OpenAPI route'],
  ['request_context: RequestContext,', 'request context'],
  ['headers: HeaderMap,', 'header extractor'],
]) requireText(products, value, label);

for (const [value, label] of [
  ['pub(crate) fn admin_product_command_idempotency_key(', 'caller-owned identity helper'],
  ['headers: &HeaderMap', 'caller-owned header input'],
  ['headers.get("Idempotency-Key")', 'caller-owned key extraction'],
  ['"commerce_admin_idempotency_key_required"', 'missing-key public code'],
  ['"commerce_admin_idempotency_key_invalid"', 'invalid-key public code'],
  ['value.len() > MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH', 'key length bound'],
  ['Ok(value)', 'exact caller key propagation'],
]) requireText(sharedIdentity, value, label);

for (const value of [
  'admin_product_command_idempotency_key<T: Serialize>',
  'serde_json::to_vec(payload)',
  'Sha256::new()',
  'digest.update(payload)',
  'commerce-admin-product:{operation}:',
]) forbidText(sharedIdentity, value, 'synthetic Product identity helper');

for (const [value, label] of [
  ['format!("commerce-admin-product:{operation}:{resource_id}")', 'resource-scoped correlation id'],
  ['.with_idempotency_key(idempotency_key)', 'idempotency propagation'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded command deadline'],
  ['request_context.channel_slug.as_deref()', 'channel propagation'],
]) requireText(sharedContext, value, label);

for (const [value, label] of [
  ['owner_code_length = error.code.chars().count()', 'bounded owner code length'],
  ['retryable = error.retryable', 'owner retryability'],
  ['error_kind', 'bounded owner error kind'],
  ['public_code = code', 'stable public code'],
  ['status = %status', 'HTTP status'],
  ['"commerce admin product owner command failed with bounded diagnostics"', 'bounded diagnostics'],
]) requireText(portMapper, value, label);

for (const value of [
  'internal_code = %error.code',
  'error = ?error',
  'error.message',
  'error.to_string()',
  'tenant_id = %context.tenant_id',
  'actor_id = %context.actor_id',
]) forbidText(portMapper, value, 'Product owner raw diagnostics');

for (const [value, label] of [
  ['product_catalog_command_runtime: rustok_product::ProductCatalogCommandRuntime', 'host Product command runtime'],
  ['shared_get::<rustok_product::ProductCatalogCommandRuntime>()', 'host runtime resolution'],
]) requireText(runtime, value, label);

requireText(
  commandPort,
  'require_policy(PortCallPolicy::write())',
  'Product owner command write admission',
);

const callerUses = products.match(/admin_product_command_idempotency_key\(&headers\)\?/g) || [];
if (callerUses.length !== 2) {
  failures.push('expected two caller-owned Product create/update identities, found ' + callerUses.length);
}

if (failures.length > 0) {
  console.error('Commerce admin Product write HTTP error-safety verification failed:');
  for (const failure of failures) console.error('- ' + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log('Commerce admin Product write owner-port, caller-idempotency, and bounded-diagnostic guards passed');
