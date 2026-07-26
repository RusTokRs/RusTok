#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_cart.rs');
const legacyFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_legacy_helpers.rs');
const source = read('crates/rustok-commerce/src/graphql/mutations/cart.rs');
const helperSource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const helperFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const pricingBoundary =
  facade.split('mod pricing_read_owner_boundary {')[1]?.split('mod rustok_cart_shim {')[0] ?? '';

for (const [value, label] of [
  ['mod pricing_read_owner_boundary {', 'pricing owner boundary module'],
  ['const PRICING_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";', 'pricing boundary constant'],
  ['fn retain_pricing_owner_context<T>(', 'pricing context retention mapper'],
  ['struct ContextualPricingReadPort {', 'contextual pricing adapter'],
  ['impl PricingReadPort for ContextualPricingReadPort', 'complete pricing adapter implementation'],
  ['inner: Arc<dyn PricingReadPort>', 'pricing owner delegation'],
  ['inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus)', 'canonical pricing constructor'],
  ['pub(crate) use pricing_read_owner_boundary::in_process_pricing_read_port as contextual_pricing_read_port;', 'shared contextual pricing export'],
  ['mod rustok_pricing_shim {', 'resolver pricing import shim'],
  ['use self::rustok_pricing_shim as rustok_pricing;', 'resolver pricing shim alias'],
  ['pub(crate) use super::pricing_read_owner_boundary::in_process_pricing_read_port;', 'resolver contextual constructor export'],
  ['pub use ::rustok_pricing::{ResolveProductPriceRequest, ResolvedPrice};', 'resolver pricing API re-export'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['channel = ?context.channel', 'channel logging'],
  ['locale = %context.locale', 'locale logging'],
  ['actor_kind = ?context.actor.kind', 'actor kind logging'],
  ['actor_id = %context.actor.id', 'actor id logging'],
  ['causation_id = ?context.causation_id', 'causation logging'],
  ['owner_code = %error.code', 'owner code logging'],
  ['owner_kind = ?error.kind', 'owner kind logging'],
  ['owner_retryable = error.retryable', 'owner retryability logging'],
  ['owner = "rustok_pricing"', 'pricing owner logging'],
  ['boundary = PRICING_GRAPHQL_OWNER_BOUNDARY', 'boundary logging'],
  ['include!("cart.rs");', 'unchanged resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const operation of [
  'resolve_product_price',
  'read_price_list_projection',
  'list_active_price_list_projections',
  'read_admin_product_pricing_projection',
  'read_storefront_product_pricing_projection',
  'preview_variant_discount',
]) {
  requireText(pricingBoundary, `"${operation}"`, `${operation} diagnostic operation`);
  requireText(pricingBoundary, `.${operation}(context, request)`, `${operation} owner delegation`);
}

for (const [pattern, expected, label] of [
  [/let error_context = context\.clone\(\);/g, 6, 'pricing context clone count'],
  [/retain_pricing_owner_context\(/g, 6, 'pricing context retention call count'],
  [/owner = "rustok_pricing"/g, 1, 'pricing owner event count'],
  [/::rustok_pricing::in_process_pricing_read_port\(db, event_bus\)/g, 1, 'canonical pricing constructor count'],
]) {
  const count = pricingBoundary.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(pricingBoundary, value, 'pricing owner context boundary');
}

for (const [value, label] of [
  ['#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;', 'safe legacy helper routing'],
]) {
  requireText(routing, value, label);
}
for (const value of ['#[path = "helpers.rs"]\nmod legacy_helpers;']) {
  forbidText(routing, value, 'unsafe legacy helper routing');
}

for (const [value, label] of [
  ['mod rustok_pricing_shim {', 'legacy pricing shim'],
  ['PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest, ResolvedPrice,', 'legacy pricing API re-export'],
  ['super::super::cart::contextual_pricing_read_port as in_process_pricing_read_port', 'shared contextual pricing constructor reuse'],
  ['use self::rustok_pricing_shim as rustok_pricing;', 'legacy pricing shim alias'],
  ['include!("helpers.rs");', 'unchanged legacy helper inclusion'],
]) {
  requireText(legacyFacade, value, label);
}
for (const value of ['::rustok_pricing::in_process_pricing_read_port']) {
  forbidText(legacyFacade, value, 'legacy canonical constructor bypass');
}

for (const [value, label] of [
  ['use rustok_pricing::{ResolveProductPriceRequest, in_process_pricing_read_port};', 'resolver pricing constructor import'],
  ['rustok_pricing::ResolvedPrice', 'resolver pricing result type'],
  ['.map_err(cart_port_error)?', 'existing public cart mapper callsites'],
  ['async fn add_storefront_cart_line_item(', 'add line item mutation'],
  ['async fn update_storefront_cart_line_item(', 'update line item mutation'],
]) {
  requireText(source, value, label);
}

const resolverPricingConstructors = source.match(/in_process_pricing_read_port\(/g) ?? [];
if (resolverPricingConstructors.length !== 2) {
  failures.push(`expected two resolver pricing constructors, found ${resolverPricingConstructors.length}`);
}
for (const [value, label] of [
  ['let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus.clone());', 'legacy repricing constructor'],
  ['storefront_pricing_port_context(tenant_id, request_context, cart.id, line_item.id)', 'legacy repricing context'],
  ['.resolve_product_price(', 'legacy pricing owner call'],
  ['.map_err(cart_port_error)?', 'legacy public cart mapper'],
]) {
  requireText(helperSource, value, label);
}
const legacyPricingConstructors = helperSource.match(/in_process_pricing_read_port\(/g) ?? [];
if (legacyPricingConstructors.length !== 1) {
  failures.push(`expected one legacy helper pricing constructor routed through the facade, found ${legacyPricingConstructors.length}`);
}

for (const [value, label] of [
  ['pub(crate) fn cart_port_error(error: PortError)', 'stable public cart mapper signature'],
  ['"CART_REQUEST_INVALID"', 'cart validation envelope'],
  ['"CART_RESOURCE_NOT_FOUND"', 'cart not-found envelope'],
  ['"CART_STATE_CONFLICT"', 'cart conflict envelope'],
  ['"CART_ACCESS_DENIED"', 'cart forbidden envelope'],
  ['"CART_TEMPORARILY_UNAVAILABLE"', 'cart availability envelope'],
  ['"CART_OPERATION_FAILED"', 'cart invariant envelope'],
  ['Some(("pricing", _)) => "rustok_pricing"', 'pricing source-owner classification'],
  ['source_owner = cart_port_source_owner(&error)', 'source-owner boundary logging'],
]) {
  requireText(helperFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront cart pricing context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL cart resolver and legacy repricing calls share contextual pricing owner diagnostics while public CART_* envelopes remain unchanged',
);
