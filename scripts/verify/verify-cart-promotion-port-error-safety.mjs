#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const lib = read('crates/rustok-cart/src/lib.rs');
const guard = read('crates/rustok-cart/src/promotion_guard.rs');
const ports = read('crates/rustok-cart/src/ports.rs');
const portContract = read('crates/rustok-api/src/ports.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

requireText(lib, 'mod promotion_guard;', 'promotion guard module');
requireText(
  lib,
  'pub use promotion_guard::guarded_cart_promotion_port as in_process_cart_promotion_port;',
  'top-level promotion constructor cutover',
);
const portExports = lib.match(/pub use ports::\{([\s\S]*?)\n\};/)?.[1] ?? '';
forbidText(
  portExports,
  'in_process_cart_promotion_port',
  'legacy constructor top-level export',
);

const contextFacts = between(
  guard,
  'fn cart_promotion_context_facts(',
  'fn cart_promotion_port_error_kind(',
  'promotion context facts',
);
const kindMapper = between(
  guard,
  'fn cart_promotion_port_error_kind(',
  'fn cart_promotion_request_facts(',
  'promotion closed error-kind mapper',
);
const requestFacts = between(
  guard,
  'fn cart_promotion_request_facts(',
  'fn cart_promotion_owner_error_facts(',
  'promotion request facts',
);
const ownerErrorFacts = between(
  guard,
  'fn cart_promotion_owner_error_facts(',
  'fn validate_cart_promotion_request(',
  'promotion owner error facts',
);
const targetValidation = between(
  guard,
  'fn validate_cart_promotion_request(',
  'fn parse_cart_promotion_tenant_id(',
  'promotion target validation',
);
const tenantParser = between(
  guard,
  'fn parse_cart_promotion_tenant_id(',
  'fn cart_promotion_context_error(',
  'promotion tenant parser',
);
const contextMapper = between(
  guard,
  'fn cart_promotion_context_error(',
  'fn cart_promotion_error(',
  'promotion context mapper',
);
const ownerMapper = between(
  guard,
  'fn cart_promotion_error(',
  'fn cart_promotion_error_code(',
  'promotion owner mapper',
);

for (const [value, label] of [
  [
    'use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};',
    'typed promotion port imports',
  ],
  ['const CART_PROMOTION_OWNER: &str = "rustok_cart.promotion";', 'promotion owner constant'],
  [
    'const CART_PROMOTION_CONTEXT_BOUNDARY: &str = "cart_promotion_context";',
    'promotion context boundary constant',
  ],
  [
    'const CART_PROMOTION_OWNER_BOUNDARY: &str = "cart_promotion_owner_service";',
    'promotion owner boundary constant',
  ],
  ['struct CartPromotionContextFacts', 'safe context model'],
  ['struct CartPromotionRequestFacts', 'safe request model'],
  ['struct CartPromotionOwnerErrorFacts', 'safe owner-error model'],
  ['fn cart_promotion_port_error_kind(', 'closed PortError kind mapper'],
  ['const READ_CART_PROMOTION_PREVIEW_OPERATION', 'preview owner operation'],
  ['const APPLY_CART_PROMOTION_OPERATION', 'apply owner operation'],
  ['service: CartService::new(db)', 'owner service construction'],
  ['let request_facts = cart_promotion_request_facts(&request);', 'request facts before delegation'],
  [
    'validate_cart_promotion_request(&context, owner_operation, &request, &request_facts)',
    'safe target validation input',
  ],
  [
    'cart_promotion_error(&context, owner_operation, &request_facts, error)',
    'safe owner mapper input',
  ],
]) requireText(guard, value, label);

const requestFactCalls =
  guard.match(/let request_facts = cart_promotion_request_facts\(&request\);/g) ?? [];
if (requestFactCalls.length !== 2) {
  failures.push(`expected preview/apply request-fact capture, found ${requestFactCalls.length}`);
}
const contextMapperCalls =
  guard.match(/cart_promotion_context_error\(&context, owner_operation, error\)/g) ?? [];
if (contextMapperCalls.length !== 2) {
  failures.push(`expected preview/apply context mapper calls, found ${contextMapperCalls.length}`);
}
const ownerMapperCalls =
  guard.match(/cart_promotion_error\(&context, owner_operation, &request_facts, error\)/g) ?? [];
if (ownerMapperCalls.length !== 2) {
  failures.push(`expected preview/apply owner mapper calls, found ${ownerMapperCalls.length}`);
}

for (const [value, label] of [
  ['tenant_id_length: context.tenant_id.chars().count()', 'tenant length fact'],
  ['actor_kind', 'actor kind fact'],
  ['actor_id_length: context.actor.id.chars().count()', 'actor length fact'],
  ['claim_count: context.claims.len()', 'claim count fact'],
  ['role_count: context.roles.len()', 'role count fact'],
  ['channel_present: context.channel.is_some()', 'channel presence fact'],
  ['channel_length: context.channel.as_ref()', 'channel length fact'],
  ['locale_length: context.locale.chars().count()', 'locale length fact'],
  ['causation_id_present: context.causation_id.is_some()', 'causation presence fact'],
  ['traceparent_present: context.traceparent.is_some()', 'trace presence fact'],
  ['idempotency_key_present: context.idempotency_key.is_some()', 'idempotency presence fact'],
  ['deadline_ms: context.deadline_ms', 'deadline fact'],
]) requireText(contextFacts, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation => "validation"', 'validation kind label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found kind label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict kind label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden kind label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable kind label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout kind label'],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', 'invariant kind label'],
]) requireText(kindMapper, value, label);

for (const [value, label] of [
  ['cart_id_non_nil: !request.cart_id.is_nil()', 'cart non-nil fact'],
  ['line_item_id_present: request.line_item_id.is_some()', 'line-item presence fact'],
  ['line_item_id_non_nil: request.line_item_id.map', 'line-item non-nil fact'],
  ['CartPromotionScopeRequest::Cart => "cart"', 'typed cart scope'],
  ['CartPromotionScopeRequest::LineItem => "line_item"', 'typed line-item scope'],
  ['CartPromotionScopeRequest::Shipping => "shipping"', 'typed shipping scope'],
  ['CartPromotionKindRequest::PercentageDiscount => "percentage_discount"', 'typed percentage kind'],
  ['CartPromotionKindRequest::FixedDiscount => "fixed_discount"', 'typed fixed kind'],
  ['source_id_present: !request.source_id.trim().is_empty()', 'source presence fact'],
  ['source_id_length: request.source_id.chars().count()', 'source length fact'],
  ['amount_text_length: request.amount.to_string().chars().count()', 'amount shape fact'],
  ['serde_json::Value::Object(values) => ("object", Some(values.len()))', 'metadata object shape'],
]) requireText(requestFacts, value, label);

for (const [value, label] of [
  ['error_variant: "validation"', 'validation variant'],
  ['validation_detail_length: Some(detail.chars().count())', 'validation detail length'],
  ['error_variant: "cart_not_found"', 'cart-not-found variant'],
  ['error_variant: "cart_line_item_not_found"', 'line-item-not-found variant'],
  ['resource_id_non_nil: Some(!id.is_nil())', 'resource non-nil fact'],
  ['error_variant: "invalid_transition"', 'transition variant'],
  ['transition_from_length: Some(from.chars().count())', 'transition from length'],
  ['transition_to_length: Some(to.chars().count())', 'transition to length'],
  ['error_variant: "database"', 'database variant'],
  ['database_error_present: true', 'database presence fact'],
  ['error_variant: "tax_boundary"', 'tax boundary variant'],
  ['tax_code_length: Some(code.chars().count())', 'tax code length'],
  ['tax_message_length: Some(message.chars().count())', 'tax message length'],
]) requireText(ownerErrorFacts, value, label);

for (const [source, label] of [
  [targetValidation, 'promotion target validation'],
  [tenantParser, 'promotion tenant parser'],
  [contextMapper, 'promotion context mapper'],
  [ownerMapper, 'promotion owner mapper'],
]) {
  for (const [value, detail] of [
    ['owner = CART_PROMOTION_OWNER', `${label} owner`],
    ['correlation_id = %context.correlation_id', `${label} correlation`],
    ['tenant_id_length = facts.tenant_id_length', `${label} tenant length`],
    ['actor_kind = facts.actor_kind', `${label} actor kind`],
    ['actor_id_length = facts.actor_id_length', `${label} actor length`],
    ['claim_count = facts.claim_count', `${label} claim count`],
    ['role_count = facts.role_count', `${label} role count`],
    ['channel_present = facts.channel_present', `${label} channel presence`],
    ['locale_length = facts.locale_length', `${label} locale length`],
    ['causation_id_present = facts.causation_id_present', `${label} causation presence`],
    ['traceparent_present = facts.traceparent_present', `${label} trace presence`],
    ['idempotency_key_present = facts.idempotency_key_present', `${label} idempotency presence`],
    ['deadline_ms = ?facts.deadline_ms', `${label} deadline`],
    ['operation = owner_operation', `${label} operation`],
  ]) requireText(source, value, detail);
}

for (const [value, label] of [
  ['scope_kind = request_facts.scope_kind', 'target scope shape'],
  ['promotion_kind = request_facts.promotion_kind', 'target kind shape'],
  ['cart_id_non_nil = request_facts.cart_id_non_nil', 'target cart shape'],
  ['line_item_id_present = request_facts.line_item_id_present', 'target line presence'],
  ['source_id_length = request_facts.source_id_length', 'target source length'],
  ['amount_text_length = request_facts.amount_text_length', 'target amount shape'],
  ['metadata_kind = request_facts.metadata_kind', 'target metadata kind'],
  ['boundary = CART_PROMOTION_CONTEXT_BOUNDARY', 'target boundary'],
]) requireText(targetValidation, value, label);

for (const [value, label] of [
  ['Uuid::parse_str(&context.tenant_id).map_err(|_|', 'opaque tenant parse rejection'],
  ['tenant_id_parse_failed = true', 'bounded tenant parse failure fact'],
  ['code = "cart.tenant_id_invalid"', 'tenant stable code'],
  ['boundary = CART_PROMOTION_CONTEXT_BOUNDARY', 'tenant boundary'],
]) requireText(tenantParser, value, label);

for (const [value, label] of [
  ['log_cart_promotion_context_rejection(context, owner_operation, &error);', 'context diagnostic before mapping'],
  ['internal_code = %error.code', 'context stable internal code'],
  ['internal_message_present = !error.message.trim().is_empty()', 'context message presence'],
  ['internal_message_length = error.message.chars().count()', 'context message length'],
  [
    'error_kind = cart_promotion_port_error_kind(&error.kind)',
    'context closed kind label',
  ],
  ['retryable = error.retryable', 'context retryability'],
  ['boundary = CART_PROMOTION_CONTEXT_BOUNDARY', 'context boundary'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'context technical severity',
  ],
  ['"cart promotion call context failed"', 'context failure event'],
  ['"cart promotion call context was rejected"', 'context rejection event'],
]) requireText(contextMapper, value, label);

for (const [value, label] of [
  ['let owner_code = cart_promotion_error_code(&error);', 'owner stable code selection'],
  ['let owner_error_facts = cart_promotion_owner_error_facts(&error);', 'owner safe fact selection'],
  ['let public_error = match &error {', 'owner public mapping'],
  ['scope_kind = request_facts.scope_kind', 'owner request scope'],
  ['promotion_kind = request_facts.promotion_kind', 'owner request kind'],
  ['cart_id_non_nil = request_facts.cart_id_non_nil', 'owner cart shape'],
  ['source_id_length = request_facts.source_id_length', 'owner source length'],
  ['amount_text_length = request_facts.amount_text_length', 'owner amount shape'],
  ['metadata_kind = request_facts.metadata_kind', 'owner metadata shape'],
  ['owner_error_variant = owner_error_facts.error_variant', 'owner error variant'],
  ['validation_detail_length = ?owner_error_facts.validation_detail_length', 'owner validation shape'],
  ['resource_id_non_nil = ?owner_error_facts.resource_id_non_nil', 'owner resource shape'],
  ['transition_from_length = ?owner_error_facts.transition_from_length', 'owner transition shape'],
  ['database_error_present = owner_error_facts.database_error_present', 'owner database fact'],
  ['tax_message_length = ?owner_error_facts.tax_message_length', 'owner tax message shape'],
  ['owner_code,', 'owner internal code'],
  ['public_code = %public_error.code', 'mapped public code'],
  [
    'error_kind = cart_promotion_port_error_kind(&public_error.kind)',
    'mapped closed public kind',
  ],
  ['retryable = public_error.retryable', 'mapped retryability'],
  ['boundary = CART_PROMOTION_OWNER_BOUNDARY', 'owner boundary'],
  ['"cart promotion owner operation failed"', 'owner failure event'],
  ['"cart promotion owner operation was rejected"', 'owner rejection event'],
  ['public_error\n}', 'same mapped error return'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['PortError::timeout(error.code, "cart promotion request context is invalid")', 'timeout context envelope'],
  ['PortError::validation(error.code, "cart promotion request context is invalid")', 'validation context envelope'],
  ['"cart.promotion_context_invalid"', 'fallback context code'],
  ['"cart promotion request is invalid"', 'promotion validation envelope'],
  ['"cart was not found"', 'cart not-found envelope'],
  ['"cart line item was not found"', 'line-item not-found envelope'],
  ['"cart promotion conflicts with the current cart state"', 'state conflict envelope'],
  ['"cart promotion tax recalculation failed"', 'tax boundary envelope'],
  ['"cart storage is temporarily unavailable"', 'storage envelope'],
]) requireText(guard, value, label);

for (const value of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'internal_tenant_id = %context.tenant_id',
  'internal_message = %error.message',
  'scope = ?request.scope',
  'source_id = %request.source_id',
  'amount = request.amount',
  'metadata = ?request.metadata',
  'parse_error = ?error',
  'error_kind = ?error.kind',
  'error_kind = ?public_error.kind',
  '\n                error = ?error,',
  '\n            error = ?error,',
]) forbidText(guard, value, 'raw promotion diagnostic field');

for (const value of [
  'crate::ports::in_process_cart_promotion_port(db)',
  '.map_err(cart_error_to_port_error)',
  'PortError::validation("cart.validation", message)',
  'format!("cart storage unavailable: {error}")',
  'format!("cart {id} not found")',
  'format!("cart line item {id} not found")',
]) forbidText(guard, value, 'promotion public error mapping');

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub actor: PortActor', 'shared actor field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub locale: String', 'shared locale field'],
  ['pub causation_id: Option<String>', 'shared causation field'],
  ['pub traceparent: Option<String>', 'shared trace field'],
  ['pub idempotency_key: Option<String>', 'shared idempotency field'],
  ['pub deadline_ms: Option<u64>', 'shared deadline field'],
  ['pub struct PortError {', 'shared port error'],
]) requireText(portContract, value, label);

requireText(
  ports,
  'impl CartPromotionPort for crate::CartService',
  'legacy promotion provider compatibility',
);

if (failures.length > 0) {
  console.error('Cart promotion port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart promotion preview/apply retain correlation and bounded parser/kind diagnostics with unchanged public envelopes',
);
