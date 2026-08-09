#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/carts.rs');
const apiPorts = read('crates/rustok-api/src/ports.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
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
  controller,
  'fn map_cart_port_error(',
  '/// Create a storefront cart',
  'storefront cart port mapper',
);
const createHandler = between(
  controller,
  'pub async fn create_cart(',
  '/// Get storefront cart',
  'create cart handler',
);
const updateLineHandler = between(
  controller,
  'pub async fn update_cart_line_item(',
  '/// Remove storefront cart line item',
  'update line item handler',
);

for (const [value, label] of [
  ['OptionalAuthContext, PortError, RequestContext, TenantContext', 'typed port error import'],
  ['port_error_to_http_error', 'shared port HTTP mapper import'],
  ['mod shipping_owner_reads;', 'mounted shipping owner read helper'],
  ['fn map_cart_port_error(', 'cart port mapper'],
  ['error = ?error', 'raw internal error logging'],
  ['owner = "rustok_cart"', 'cart owner logging'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['cart_id = ?cart_id', 'optional cart logging'],
  ['error_kind = ?error.kind', 'typed error-kind logging'],
  ['retryable = error.retryable', 'retryability logging'],
  ['public_code = %public.code', 'public-code logging'],
  ['status = %public.status', 'status logging'],
  ['boundary = "commerce_storefront_cart_http"', 'cart HTTP boundary logging'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['let public = port_error_to_http_error(error.clone());', 'shared safe mapping'],
  ['"storefront cart port operation failed"', 'boundary log message'],
  ['public\n}', 'mapped error return'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'commerce_operation_failed',
  'error.message',
  'err.to_string()',
  'error.to_string()',
  'map_err(rustok_web::port_error_to_http_error)',
]) {
  forbidText(controller, value, 'unsafe storefront cart public conversion');
}

for (const [value, label] of [
  ['"commerce_store_cart_context_invalid"', 'cart-context validation code'],
  ['"Currency code is required unless it can be resolved from region or country"', 'static currency message'],
  ['super::resolve_context_for_db(', 'store context resolution'],
  ['context.currency_code', 'resolved currency preference'],
  ['CartStorefrontCreateRequest {', 'typed create request'],
  ['customer_id,', 'customer identity propagation'],
  ['region_id: context.region.as_ref().map(|region| region.id)', 'resolved region propagation'],
  ['locale_code: Some(context.locale.clone())', 'resolved locale propagation'],
  ['channel_id: request_context.channel_id', 'channel ID propagation'],
  ['channel_slug: request_context.channel_slug.clone()', 'channel slug propagation'],
  ['StatusCode::CREATED', 'created response status'],
]) {
  requireText(createHandler, value, label);
}

for (const operation of [
  '"create_cart"',
  '"get_cart"',
  '"update_cart_context_read"',
  '"add_cart_line_item_read"',
  '"add_cart_line_item"',
  '"update_cart_line_item_read"',
  '"update_cart_line_item_pricing"',
  '"update_cart_line_item_quantity"',
  '"remove_cart_line_item_read"',
  '"remove_cart_line_item"',
]) {
  requireText(controller, operation, 'cart diagnostic operation label');
}

const mapperUses = controller.match(/map_cart_port_error\(/g) ?? [];
if (mapperUses.length !== 11) {
  failures.push(`expected mapper definition plus ten cart-owned callsites, found ${mapperUses.length}`);
}
const directSharedMapperUses = controller.match(/\.map_err\(port_error_to_http_error\)\?/g) ?? [];
if (directSharedMapperUses.length !== 1) {
  failures.push(
    `expected exactly one direct shared mapper use for the pricing owner, found ${directSharedMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['pub async fn create_cart(', 'create handler'],
  ['pub async fn get_cart(', 'get handler'],
  ['pub async fn update_cart_context(', 'context update handler'],
  ['pub async fn add_cart_line_item(', 'add line handler'],
  ['pub async fn update_cart_line_item(', 'update line handler'],
  ['pub async fn remove_cart_line_item(', 'remove line handler'],
  ['ensure_storefront_channel_enabled_for_db(', 'channel guard'],
  ['current_customer_id_for_db(', 'customer lookup'],
  ['ensure_store_cart_access(', 'cart ownership guard'],
  ['runtime.shipping_option_read_port()', 'host-selected Fulfillment shipping read port'],
  ['shipping_owner_reads::enrich_storefront_cart(', 'cart shipping enrichment'],
  ['shipping_owner_reads::apply_cart_context_patch(', 'cart context shipping validation'],
  ['storefront_cart_port_context(', 'cart port context'],
]) {
  requireText(controller, value, label);
}

for (const value of [
  'super::enrich_storefront_cart_for_db(',
  'super::apply_cart_context_patch_for_db(',
  'FulfillmentService::new(',
]) {
  forbidText(controller, value, 'stale mounted storefront shipping construction');
}

for (const [value, label] of [
  ['CartStorefrontReadRequest { cart_id: id }', 'typed cart read'],
  ['CartStorefrontAddLineItemRequest {', 'typed add-line request'],
  ['CartStorefrontLineItemPricingRequest {', 'typed pricing update request'],
  ['CartStorefrontLineItemQuantityRequest {', 'typed quantity update request'],
  ['CartStorefrontRemoveLineItemRequest {', 'typed remove-line request'],
  ['resolve_store_line_item_input(', 'line-item resolution'],
  ['validate_store_line_item_quantity(', 'inventory quantity validation'],
  ['build_store_pricing_context(', 'pricing context'],
  ['ResolveProductPriceRequest {', 'typed pricing request'],
  ['storefront_cart_pricing_update(', 'pricing snapshot update'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['.resolve_product_price(', 'pricing resolution call'],
  ['.map_err(port_error_to_http_error)?', 'pricing owner shared mapper'],
  ['product_id: existing', 'existing product identity'],
  ['variant_id,', 'variant identity'],
  ['quantity: pricing_context.quantity', 'pricing quantity'],
  ['currency_code: pricing_context.currency_code', 'pricing currency'],
]) {
  requireText(updateLineHandler, value, label);
}

for (const [content, value, label] of [
  [apiPorts, 'pub struct PortError {', 'owner port error type'],
  [apiPorts, 'pub kind: PortErrorKind', 'owner port kind'],
  [apiPorts, 'pub code: String', 'owner port code'],
  [apiPorts, 'pub message: String', 'owner port message'],
  [apiPorts, 'pub retryable: bool', 'owner port retryability'],
  [webErrors, 'pub fn port_error_to_http_error(error: PortError)', 'shared port HTTP mapper'],
  [webErrors, 'PortErrorKind::Validation => StatusCode::BAD_REQUEST', 'validation status'],
  [webErrors, 'PortErrorKind::NotFound => StatusCode::NOT_FOUND', 'not-found status'],
  [webErrors, 'PortErrorKind::Conflict => StatusCode::CONFLICT', 'conflict status'],
  [webErrors, 'PortErrorKind::Forbidden => StatusCode::FORBIDDEN', 'forbidden status'],
  [webErrors, 'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  [webErrors, 'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT', 'timeout status'],
  [webErrors, 'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR', 'invariant status'],
  [webErrors, '"The requested service is temporarily unavailable"', 'safe unavailable message'],
  [webErrors, '"The requested operation could not be completed"', 'safe invariant message'],
]) {
  requireText(content, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront cart HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Storefront cart handlers use typed safe public envelopes, host-selected shipping reads, and diagnostics');
