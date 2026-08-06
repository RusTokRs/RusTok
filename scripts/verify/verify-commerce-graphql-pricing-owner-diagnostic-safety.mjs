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

const pricingBoundary = between(
  facade,
  'mod pricing_read_owner_boundary {',
  'pub(crate) use pricing_read_owner_boundary::in_process_pricing_read_port',
  'pricing owner boundary module',
);

for (const [value, label] of [
  ['PortActorKind, PortContext, PortError', 'pricing diagnostic imports'],
  ['const PRICING_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";', 'pricing boundary'],
  ['struct PricingOwnerDiagnosticContext {', 'bounded pricing context'],
  ['impl From<&PortContext> for PricingOwnerDiagnosticContext', 'pricing context projection'],
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
  ['struct PricingOwnerDiagnosticError;', 'redacted pricing diagnostic token'],
  ['impl std::fmt::Debug for PricingOwnerDiagnosticError', 'custom pricing diagnostic Debug'],
  ['formatter.write_str("redacted")', 'redacted pricing diagnostic output'],
  ['fn actor_kind_name(kind: &PortActorKind)', 'actor kind helper'],
  ['fn identity_text_shape(value: &str)', 'identity shape helper'],
  ['fn text_shape(value: &str)', 'text shape helper'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
  ['fn retain_pricing_owner_context<T>(', 'pricing context retention mapper'],
  ['let diagnostic_context = PricingOwnerDiagnosticContext::from(context);', 'context projection'],
  ['let owner_code = error.code.clone();', 'owner code projection'],
  ['let owner_kind = error.kind.clone();', 'owner kind projection'],
  ['let owner_retryable = error.retryable;', 'owner retryability projection'],
  ['let owner_message_shape = text_shape(error.message.as_str());', 'owner message shape'],
  ['let owner_message_len = error.message.len();', 'owner message length'],
  ['let diagnostic_error = PricingOwnerDiagnosticError;', 'diagnostic shadow token'],
  ['tracing::error!(', 'pricing owner error event'],
  ['error = ?diagnostic_error', 'redacted diagnostic field'],
  ['owner = "rustok_pricing"', 'pricing owner field'],
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
  ['boundary = PRICING_GRAPHQL_OWNER_BOUNDARY', 'boundary field'],
  ['"commerce GraphQL storefront cart pricing owner call failed"', 'static pricing event'],
  ['struct ContextualPricingReadPort {', 'contextual pricing adapter'],
  ['inner: Arc<dyn PricingReadPort>', 'canonical pricing port delegation'],
  ['inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus)', 'canonical constructor'],
]) {
  requireText(pricingBoundary, value, label);
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
  forbidText(pricingBoundary, value, 'raw pricing owner diagnostic');
}

const operations = [
  'resolve_product_price',
  'read_price_list_projection',
  'list_active_price_list_projections',
  'read_admin_product_pricing_projection',
  'read_storefront_product_pricing_projection',
  'preview_variant_discount',
];
for (const operation of operations) {
  requireText(pricingBoundary, `"${operation}"`, `${operation} diagnostic operation`);
  requireText(pricingBoundary, `.${operation}(context, request)`, `${operation} owner delegation`);
}

for (const [pattern, expected, label] of [
  [/let error_context = context\.clone\(\);/g, 6, 'owner context clone count'],
  [/retain_pricing_owner_context\(/g, 6, 'pricing context retention call count'],
  [/owner = "rustok_pricing"/g, 1, 'pricing owner event count'],
  [/::rustok_pricing::in_process_pricing_read_port\(db, event_bus\)/g, 1, 'constructor count'],
]) {
  const count = pricingBoundary.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

const projectionIndex = pricingBoundary.indexOf(
  'let diagnostic_context = PricingOwnerDiagnosticContext::from(context);',
);
const ownerFactsIndex = pricingBoundary.indexOf('let owner_code = error.code.clone();');
const tokenIndex = pricingBoundary.indexOf('let diagnostic_error = PricingOwnerDiagnosticError;');
const eventIndex = pricingBoundary.indexOf('tracing::error!(');
const returnIndex = pricingBoundary.indexOf('\n            error\n        })', eventIndex);
if (
  !(
    projectionIndex >= 0 &&
    projectionIndex < ownerFactsIndex &&
    ownerFactsIndex < tokenIndex &&
    tokenIndex < eventIndex &&
    eventIndex < returnIndex
  )
) {
  failures.push('pricing owner error must be projected, redacted, diagnosed, and returned in order');
}

for (const [value, label] of [
  ['fn cart_port_source_owner(error: &PortError)', 'source-owner classifier'],
  ['Some(("pricing", _)) => "rustok_pricing"', 'pricing source-owner classification'],
  ['pub(crate) fn cart_port_error(error: PortError)', 'downstream public mapper'],
  ['let source_owner = cart_port_source_owner(&error);', 'downstream source projection'],
  ['source_owner,', 'downstream source field'],
  ['"CART_REQUEST_INVALID"', 'validation envelope'],
  ['"CART_RESOURCE_NOT_FOUND"', 'not-found envelope'],
  ['"CART_STATE_CONFLICT"', 'conflict envelope'],
  ['"CART_ACCESS_DENIED"', 'forbidden envelope'],
  ['"CART_TEMPORARILY_UNAVAILABLE"', 'availability envelope'],
  ['"CART_OPERATION_FAILED"', 'invariant envelope'],
]) {
  requireText(helperFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL pricing owner diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL Pricing read-owner calls retain exact delegation and public envelopes while diagnostics expose only bounded context and owner facts',
);
