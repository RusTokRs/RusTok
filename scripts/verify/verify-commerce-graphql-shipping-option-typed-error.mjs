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
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs',
);
const safeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const legacySource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');

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
    '#[path = "typed_shipping_option_helper.rs"]\nmod typed_shipping_option_helper;',
    'private typed helper module',
  ],
  [
    moduleSource,
    '#[allow(dead_code)]\n#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;',
    'private compatibility facade allowance',
  ],
  [
    moduleSource,
    '#[allow(dead_code)]\n#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;',
    'private legacy helper allowance',
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
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['enum ShippingOptionFailureKind {', 'typed outcome enum'],
  ['MultipleDeliveryGroups,', 'multiple-group outcome'],
  ['OwnerValidation,', 'owner validation outcome'],
  ['OwnerNotFound,', 'owner not-found outcome'],
  ['OwnerConflict,', 'owner conflict outcome'],
  ['StorageUnavailable,', 'storage outcome'],
  ['CurrencyMismatch,', 'currency outcome'],
  ['ChannelUnavailable,', 'channel outcome'],
  ['ProfileIncompatible,', 'profile outcome'],
  ['shipping_option_id: Option<Uuid>', 'typed option identity'],
  ['profile_slug_length: Option<usize>', 'bounded profile fact'],
  ['option_currency_code_length: Option<usize>', 'bounded currency fact'],
  ['owner_error: Option<FulfillmentError>', 'typed owner cause'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'option not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition conflict mapping'],
  ['FulfillmentError::Database(_)', 'database mapping'],
  ['"fulfillment.validation"', 'validation internal code'],
  ['"fulfillment.shipping_option_not_found"', 'option internal code'],
  ['"fulfillment.fulfillment_not_found"', 'fulfillment internal code'],
  ['"fulfillment.invalid_transition"', 'transition internal code'],
  ['"fulfillment.database_unavailable"', 'database internal code'],
  ['"shipping_selection.multiple_delivery_groups"', 'group internal code'],
  ['"shipping_selection.currency_mismatch"', 'currency internal code'],
  ['"shipping_selection.channel_unavailable"', 'channel internal code'],
  ['"shipping_selection.profile_incompatible"', 'profile internal code'],
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
  ['owner = failure.source_owner', 'truthful source owner'],
  ['owner_operation = failure.source_operation', 'truthful source operation'],
  ['internal_code = failure.internal_code', 'internal code'],
  ['internal_kind = failure.internal_kind', 'internal kind'],
  ['internal_retryable = failure.internal_retryable', 'internal retryability'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['cart_id = %cart_id', 'cart identity'],
  ['shipping_option_id = ?failure.shipping_option_id', 'option identity'],
  ['selection_count,', 'selection count'],
  ['delivery_group_count,', 'delivery group count'],
  ['requested_currency_code_length,', 'requested currency length'],
  ['option_currency_code_length = ?failure.option_currency_code_length', 'owner currency length'],
  ['profile_slug_length = ?failure.profile_slug_length', 'profile length'],
  ['channel_slug_length = ?channel_slug_length', 'channel length'],
  ['requested_locale_length = ?requested_locale_length', 'requested locale length'],
  ['tenant_default_locale_length = ?tenant_default_locale_length', 'default locale length'],
  ['public_code = "SHIPPING_OPTION_INVALID"', 'public code diagnostic'],
  ['public_retryable = false', 'public retryability diagnostic'],
  ['boundary = STOREFRONT_SHIPPING_OPTION_GRAPHQL_BOUNDARY', 'stable boundary'],
  ['error = ?technical_owner_error', 'technical owner cause'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
  ['public_graphql_error()', 'single stable envelope return'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['let requested_selection_count = shipping_selections', 'selection count source'],
  ['.map(|selections| selections.len())', 'safe selection counting'],
  ['if let Some(shipping_option_id) = selected_shipping_option_id', 'typed selected option branch'],
  ['FulfillmentService::new(db.clone())', 'single owner service construction'],
  ['.get_shipping_option(', 'owner lookup'],
  ['ShippingOptionFailure::owner(shipping_option_id, error)', 'typed owner mapping'],
  ['ShippingOptionFailure::currency_mismatch(', 'currency mapping'],
  ['ShippingOptionFailure::channel_unavailable(option.id)', 'channel mapping'],
  ['ShippingOptionFailure::profile_incompatible(', 'profile mapping'],
  ['is_shipping_option_compatible_with_profiles', 'profile compatibility policy'],
  ['is_metadata_visible_for_public_channel', 'channel visibility policy'],
]) {
  requireText(mountedValidator, value, label);
}

const ownerLookups = mountedValidator.match(/\.get_shipping_option\(/g) ?? [];
if (ownerLookups.length !== 1) {
  failures.push(`expected one fulfillment owner lookup, found ${ownerLookups.length}`);
}
const publicEnvelopeDefinitions = typedSource.match(/fn public_graphql_error\(\)/g) ?? [];
if (publicEnvelopeDefinitions.length !== 1) {
  failures.push(`expected one public envelope definition, found ${publicEnvelopeDefinitions.length}`);
}
const mountedOverrides = layeredSource.match(/validate_selected_shipping_option/g) ?? [];
if (mountedOverrides.length !== 1) {
  failures.push(`expected one mounted typed shipping-option override, found ${mountedOverrides.length}`);
}

for (const value of [
  'format!("Shipping option {} uses currency {}, expected {}"',
  'format!("Shipping option {} is not available for the current channel"',
  'format!("Shipping option {} is not compatible with shipping profile {}"',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'format!("{error:?}")',
  'detail.contains(',
  '.expect(',
  '.unwrap()',
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
  '✔ mounted storefront shipping-option validation uses typed local outcomes and one stable GraphQL envelope',
);
