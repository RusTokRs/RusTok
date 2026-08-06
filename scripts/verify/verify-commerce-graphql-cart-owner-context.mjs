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
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const cartBoundary = between(
  facade,
  'mod cart_storefront_owner_boundary {',
  'mod pricing_read_owner_boundary {',
  'cart owner boundary module',
);

for (const [value, label] of [
  ['mod cart_storefront_owner_boundary {', 'cart owner boundary module'],
  ['const CART_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";', 'cart boundary constant'],
  ['struct ContextualCartStorefrontPort {', 'contextual cart adapter'],
  ['impl CartStorefrontPort for ContextualCartStorefrontPort', 'cart adapter implementation'],
  ['inner: Arc<dyn CartStorefrontPort>', 'owner port delegation'],
  ['inner: ::rustok_cart::in_process_cart_storefront_port(db)', 'canonical cart owner constructor'],
  ['mod rustok_cart_shim {', 'cart import shim'],
  ['use self::rustok_cart_shim as rustok_cart;', 'resolver cart shim alias'],
  [
    'pub(crate) use super::cart_storefront_owner_boundary::in_process_cart_storefront_port;',
    'contextual constructor export',
  ],
  ['include!("cart.rs");', 'unchanged resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['PortActorKind, PortContext, PortError', 'cart diagnostic imports'],
  ['struct CartOwnerDiagnosticContext {', 'bounded cart owner context'],
  ['impl From<&PortContext> for CartOwnerDiagnosticContext', 'cart context projection'],
  ['tenant_id_shape: identity_text_shape(context.tenant_id.as_str())', 'tenant identity shape'],
  ['actor_kind: actor_kind_name(&context.actor.kind)', 'actor kind projection'],
  ['actor_id_shape: identity_text_shape(context.actor.id.as_str())', 'actor identity shape'],
  ['claim_count: context.claims.len()', 'claim count'],
  ['role_count: context.roles.len()', 'role count'],
  ['channel_shape: optional_text_shape(context.channel.as_deref())', 'channel shape'],
  ['locale_shape: text_shape(context.locale.as_str())', 'locale shape'],
  ['correlation_id_shape: text_shape(context.correlation_id.as_str())', 'correlation shape'],
  ['causation_id_shape: optional_text_shape(context.causation_id.as_deref())', 'causation shape'],
  ['traceparent_shape: optional_text_shape(context.traceparent.as_deref())', 'trace shape'],
  [
    'idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref())',
    'idempotency shape',
  ],
  ['deadline_ms: context.deadline_ms', 'deadline fact'],
  ['struct CartOwnerDiagnosticError;', 'redacted cart diagnostic token'],
  ['impl std::fmt::Debug for CartOwnerDiagnosticError', 'custom cart diagnostic Debug'],
  ['formatter.write_str("redacted")', 'redacted cart diagnostic output'],
  ['fn actor_kind_name(kind: &PortActorKind)', 'actor kind helper'],
  ['fn identity_text_shape(value: &str)', 'identity shape helper'],
  ['fn text_shape(value: &str)', 'text shape helper'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
  ['fn retain_cart_owner_context<T>(', 'context retention mapper'],
  ['let diagnostic_context = CartOwnerDiagnosticContext::from(context);', 'context projection'],
  ['let owner_code = error.code.clone();', 'owner code projection'],
  ['let owner_kind = error.kind.clone();', 'owner kind projection'],
  ['let owner_retryable = error.retryable;', 'owner retryability projection'],
  ['let owner_message_shape = text_shape(error.message.as_str());', 'owner message shape'],
  ['let owner_message_len = error.message.len();', 'owner message length'],
  ['let diagnostic_error = CartOwnerDiagnosticError;', 'diagnostic shadow token'],
  ['tracing::error!(', 'cart owner error event'],
  ['error = ?diagnostic_error', 'redacted diagnostic field'],
  ['owner = "rustok_cart"', 'cart owner logging'],
  ['tenant_id_shape = diagnostic_context.tenant_id_shape', 'tenant shape field'],
  ['actor_kind = diagnostic_context.actor_kind', 'actor kind field'],
  ['actor_id_shape = diagnostic_context.actor_id_shape', 'actor identity shape field'],
  ['claim_count = diagnostic_context.claim_count', 'claim count field'],
  ['role_count = diagnostic_context.role_count', 'role count field'],
  ['channel_shape = diagnostic_context.channel_shape', 'channel shape field'],
  ['locale_shape = diagnostic_context.locale_shape', 'locale shape field'],
  ['correlation_id_shape = diagnostic_context.correlation_id_shape', 'correlation shape field'],
  ['causation_id_shape = diagnostic_context.causation_id_shape', 'causation shape field'],
  ['traceparent_shape = diagnostic_context.traceparent_shape', 'trace shape field'],
  [
    'idempotency_key_shape = diagnostic_context.idempotency_key_shape',
    'idempotency shape field',
  ],
  ['deadline_ms = ?diagnostic_context.deadline_ms', 'deadline field'],
  ['operation,', 'exact operation field'],
  ['owner_code = %owner_code', 'owner code field'],
  ['owner_message_shape,', 'owner message shape field'],
  ['owner_message_len,', 'owner message length field'],
  ['owner_kind = ?owner_kind', 'owner kind field'],
  ['owner_retryable,', 'owner retryability field'],
  ['boundary = CART_GRAPHQL_OWNER_BOUNDARY', 'boundary logging'],
  ['"commerce GraphQL storefront cart owner call failed"', 'static cart owner event'],
]) {
  requireText(cartBoundary, value, label);
}

for (const value of [
  'error = ?error',
  'correlation_id = %context.correlation_id',
  'correlation_id = ?context.correlation_id',
  'tenant_id = %context.tenant_id',
  'tenant_id = ?context.tenant_id',
  'channel = ?context.channel',
  'channel = %context.channel',
  'locale = %context.locale',
  'locale = ?context.locale',
  'actor_kind = ?context.actor.kind',
  'actor_id = %context.actor.id',
  'actor_id = ?context.actor.id',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'internal_message =',
  'owner_message =',
  'message = %error.message',
  'message = ?error.message',
]) {
  forbidText(cartBoundary, value, 'raw cart owner diagnostic');
}

const projectionIndex = cartBoundary.indexOf(
  'let diagnostic_context = CartOwnerDiagnosticContext::from(context);',
);
const ownerFactsIndex = cartBoundary.indexOf('let owner_code = error.code.clone();');
const tokenIndex = cartBoundary.indexOf('let diagnostic_error = CartOwnerDiagnosticError;');
const eventIndex = cartBoundary.indexOf('tracing::error!(');
const returnIndex = cartBoundary.indexOf('\n            error\n        })', eventIndex);
if (
  !(
    projectionIndex >= 0 &&
    projectionIndex < ownerFactsIndex &&
    ownerFactsIndex < tokenIndex &&
    tokenIndex < eventIndex &&
    eventIndex < returnIndex
  )
) {
  failures.push('cart owner error must be projected, redacted, diagnosed, and returned in order');
}
if ((cartBoundary.match(/tracing::error!\(/g) ?? []).length !== 1) {
  failures.push('expected one cart owner diagnostic event');
}
if ((cartBoundary.match(/error = \?diagnostic_error/g) ?? []).length !== 1) {
  failures.push('expected one redacted cart owner diagnostic field');
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
  requireText(cartBoundary, `"${operation}"`, `${operation} diagnostic operation`);
  requireText(cartBoundary, `.${operation}(context, request)`, `${operation} owner delegation`);
}

for (const [pattern, expected, label] of [
  [/let error_context = context\.clone\(\);/g, 8, 'owner context clone count'],
  [/retain_cart_owner_context\(/g, 8, 'context retention call count'],
  [/owner = "rustok_cart"/g, 1, 'cart owner event count'],
  [/::rustok_cart::in_process_cart_storefront_port\(db\)/g, 1, 'canonical constructor count'],
]) {
  const count = cartBoundary.match(pattern)?.length ?? 0;
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
  ['let source_owner = cart_port_source_owner(&error);', 'source owner projection'],
  ['source_owner,', 'source owner boundary logging'],
]) {
  requireText(helperFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront cart owner context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL storefront cart owner calls retain exact delegation and public envelopes while diagnostics expose only bounded context and owner facts',
);
