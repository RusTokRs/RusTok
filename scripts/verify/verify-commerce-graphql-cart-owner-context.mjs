#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_cart.rs');
const source = read('crates/rustok-commerce/src/graphql/mutations/cart.rs');
const helperFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['mod cart_storefront_owner_boundary {', 'cart owner boundary module'],
  ['const CART_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";', 'cart boundary constant'],
  ['fn retain_cart_owner_context<T>(', 'context retention mapper'],
  ['struct ContextualCartStorefrontPort {', 'contextual cart adapter'],
  ['impl CartStorefrontPort for ContextualCartStorefrontPort', 'cart adapter implementation'],
  ['inner: Arc<dyn CartStorefrontPort>', 'owner port delegation'],
  ['inner: ::rustok_cart::in_process_cart_storefront_port(db)', 'canonical cart owner constructor'],
  ['mod rustok_cart_shim {', 'cart import shim'],
  ['use self::rustok_cart_shim as rustok_cart;', 'resolver cart shim alias'],
  ['pub(crate) use super::cart_storefront_owner_boundary::in_process_cart_storefront_port;', 'contextual constructor export'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['channel = ?context.channel', 'channel logging'],
  ['locale = %context.locale', 'locale logging'],
  ['actor_kind = ?context.actor.kind', 'actor kind logging'],
  ['actor_id = %context.actor.id', 'actor id logging'],
  ['causation_id = ?context.causation_id', 'causation logging'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency logging'],
  ['owner_code = %error.code', 'owner code logging'],
  ['owner_kind = ?error.kind', 'owner kind logging'],
  ['owner_retryable = error.retryable', 'owner retryability logging'],
  ['owner = "rustok_cart"', 'cart owner logging'],
  ['boundary = CART_GRAPHQL_OWNER_BOUNDARY', 'boundary logging'],
  ['include!("cart.rs");', 'unchanged resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const operation of [
  'read_storefront_cart',
  'create_storefront_cart',
  'add_storefront_line_item',
  'update_storefront_context',
  'update_storefront_line_item_quantity',
  'update_storefront_line_item_pricing',
  'remove_storefront_line_item',
  'reprice_storefront_line_items',
]) {
  requireText(facade, `"${operation}"`, `${operation} diagnostic operation`);
  requireText(facade, `.${operation}(context, request)`, `${operation} owner delegation`);
}

for (const [pattern, expected, label] of [
  [/let error_context = context\.clone\(\);/g, 8, 'owner context clone count'],
  [/retain_cart_owner_context\(/g, 8, 'context retention call count'],
  [/owner = "rustok_cart"/g, 1, 'cart owner event count'],
  [/::rustok_cart::in_process_cart_storefront_port\(db\)/g, 1, 'canonical constructor count'],
]) {
  const count = facade.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(facade, value, 'cart owner context boundary');
}

for (const [value, label] of [
  ['in_process_cart_storefront_port,', 'resolver owner constructor import'],
  ['.map_err(cart_port_error)?', 'existing cart public mapper callsites'],
  ['async fn create_storefront_cart(', 'create cart mutation'],
  ['async fn add_storefront_cart_line_item(', 'add line item mutation'],
  ['async fn update_storefront_cart_context(', 'update cart context mutation'],
  ['async fn update_storefront_cart_line_item(', 'update line item mutation'],
  ['async fn remove_storefront_cart_line_item(', 'remove line item mutation'],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['pub(crate) fn cart_port_error(error: PortError)', 'stable public cart mapper signature'],
  ['"CART_REQUEST_INVALID"', 'cart validation envelope'],
  ['"CART_RESOURCE_NOT_FOUND"', 'cart not-found envelope'],
  ['"CART_STATE_CONFLICT"', 'cart conflict envelope'],
  ['"CART_ACCESS_DENIED"', 'cart forbidden envelope'],
  ['"CART_TEMPORARILY_UNAVAILABLE"', 'cart availability envelope'],
  ['"CART_OPERATION_FAILED"', 'cart invariant envelope'],
  ['source_owner = cart_port_source_owner(&error)', 'source owner boundary logging'],
]) {
  requireText(helperFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront cart owner context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL storefront cart owner calls retain the original PortContext and exact operation while public CART_* envelopes remain unchanged',
);
