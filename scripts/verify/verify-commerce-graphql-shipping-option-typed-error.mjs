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

const moduleSource = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const layeredSource = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
);
const contextSource = read(
  'crates/rustok-commerce/src/graphql/mutations/shipping_option_read_context.rs',
);
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs',
);
const safeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const legacySource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const ownerSource = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const ownerRoot = read('crates/rustok-fulfillment/src/lib.rs');

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

const publicEnvelope = between(
  typedSource,
  'fn public_graphql_error()',
  '#[allow(clippy::too_many_arguments)]\nfn shipping_option_graphql_error(',
  'public GraphQL envelope',
);
const mapper = between(
  typedSource,
  'fn shipping_option_graphql_error(',
  'fn current_shipping_selections(',
  'typed shipping option mapper',
);
const validatorStart = typedSource.indexOf(
  'pub(crate) async fn validate_selected_shipping_option(',
);
const mountedValidator = validatorStart >= 0 ? typedSource.slice(validatorStart) : '';
if (validatorStart < 0) failures.push('mounted validator: unable to isolate source block');

for (const [source, value, label] of [
  [
    moduleSource,
    '#[path = "shipping_option_read_context.rs"]\nmod shipping_option_read_context;',
    'private read context seam',
  ],
  [
    moduleSource,
    '#[path = "typed_shipping_option_helper.rs"]\nmod typed_shipping_option_helper;',
    'private typed helper module',
  ],
  [
    layeredSource,
    'pub(crate) use super::typed_shipping_option_helper::validate_selected_shipping_option;',
    'mounted typed override',
  ],
  [safeSource, 'pub(crate) async fn validate_selected_shipping_option(', 'compatibility facade'],
  [
    safeSource,
    'super::legacy_helpers::validate_selected_shipping_option(',
    'compatibility delegation',
  ],
  [legacySource, 'pub(crate) async fn validate_selected_shipping_option(', 'legacy source'],
  [ownerSource, 'pub trait ShippingOptionReadPort: Send + Sync {', 'owner read port'],
  [
    ownerRoot,
    'in_process_shipping_option_read_port,',
    'canonical owner root factory export',
  ],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['PortActor::service("rustok-commerce.storefront-shipping")', 'service actor'],
  ['format!("storefront-shipping:{operation}:{cart_id}")', 'correlation identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['context.clone().with_channel(channel)', 'channel propagation'],
  [
    'rustok_fulfillment::in_process_shipping_option_read_port(db)',
    'canonical owner factory delegation',
  ],
]) {
  requireText(contextSource, value, label);
}

for (const [value, label] of [
  ['enum ShippingOptionFailureKind {', 'typed outcome enum'],
  ['MultipleDeliveryGroups,', 'multiple-group outcome'],
  ['OwnerValidation,', 'owner validation outcome'],
  ['OwnerNotFound,', 'owner not-found outcome'],
  ['OwnerConflict,', 'owner conflict outcome'],
  ['OwnerForbidden,', 'owner forbidden outcome'],
  ['StorageUnavailable,', 'availability outcome'],
  ['OwnerInvariant,', 'invariant outcome'],
  ['CurrencyMismatch,', 'currency outcome'],
  ['ChannelUnavailable,', 'channel outcome'],
  ['ProfileIncompatible,', 'profile outcome'],
  ['owner_error: Option<PortError>', 'typed owner cause'],
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Forbidden', 'forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'availability mapping'],
  ['PortErrorKind::InvariantViolation', 'invariant mapping'],
  ['source_operation: "read_shipping_option_projection"', 'owner operation'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  ['async_graphql::Error::new("Selected shipping option is invalid")', 'stable public message'],
  ['extensions.set("code", "SHIPPING_OPTION_INVALID")', 'stable public code'],
  ['extensions.set("retryable", false)', 'stable public retryability'],
]) {
  requireText(publicEnvelope, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained owner context'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['context_channel_length = context.channel.as_deref().map(str::len)', 'bounded channel context'],
  ['context_locale_length = context.locale.len()', 'bounded locale context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['owner = failure.source_owner', 'truthful source owner'],
  ['owner_operation = failure.source_operation', 'truthful source operation'],
  ['internal_code = %failure.internal_code', 'internal code'],
  ['internal_kind = failure.internal_kind', 'internal kind'],
  ['internal_retryable = failure.internal_retryable', 'internal retryability'],
  ['shipping_option_id = ?failure.shipping_option_id', 'option identity'],
  ['public_code = "SHIPPING_OPTION_INVALID"', 'public code diagnostic'],
  ['public_retryable = false', 'public retryability diagnostic'],
  ['error = ?technical_owner_error', 'technical owner cause'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
  ['public_graphql_error()', 'single stable envelope return'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  [
    'storefront_shipping_option_read_context(',
    'owner context construction',
  ],
  [
    'storefront_shipping_option_read_port(db.clone())',
    'owner read port construction',
  ],
  ['ReadShippingOptionProjectionRequest {', 'typed read request'],
  ['.read_shipping_option_projection(', 'owner projection read'],
  ['owner_context.clone(),', 'delegated owner context'],
  ['ShippingOptionFailure::owner(shipping_option_id, error)', 'typed owner mapping'],
  ['ShippingOptionFailure::currency_mismatch(', 'currency mapping'],
  ['ShippingOptionFailure::channel_unavailable(option.id)', 'channel mapping'],
  ['ShippingOptionFailure::profile_incompatible(', 'profile mapping'],
  ['is_shipping_option_compatible_with_profiles', 'profile compatibility policy'],
  ['is_metadata_visible_for_public_channel', 'channel visibility policy'],
]) {
  requireText(mountedValidator, value, label);
}

const ownerLookups = mountedValidator.match(/\.read_shipping_option_projection\(/g) ?? [];
if (ownerLookups.length !== 1) {
  failures.push(`expected one fulfillment owner projection call, found ${ownerLookups.length}`);
}
const mountedOverrides = layeredSource.match(/validate_selected_shipping_option/g) ?? [];
if (mountedOverrides.length !== 1) {
  failures.push(`expected one mounted typed shipping-option override, found ${mountedOverrides.length}`);
}

for (const value of [
  'FulfillmentService::new(',
  '.get_shipping_option(',
  'FulfillmentError',
  'format!("Shipping option {} uses currency {}, expected {}"',
  'format!("Shipping option {} is not available for the current channel"',
  'format!("Shipping option {} is not compatible with shipping profile {}"',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'format!("{error:?}")',
  'detail.contains(',
  'error.message',
  'currency_code = %',
  'currency_code = ?',
  'public_channel_slug = %',
  'public_channel_slug = ?',
  'requested_locale = %',
  'requested_locale = ?',
  'tenant_default_locale = %',
  'tenant_default_locale = ?',
  'shipping_profile_slug = %',
  'shipping_profile_slug = ?',
]) {
  forbidText(typedSource, value, 'typed storefront shipping option boundary');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL typed shipping option verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted storefront shipping-option validation uses the fulfillment read port, retained context, typed local outcomes, and one stable GraphQL envelope',
);
