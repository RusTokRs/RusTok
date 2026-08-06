#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shipping = read('crates/rustok-commerce/src/storefront_shipping.rs');
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

const typedEnrichment = between(
  shipping,
  'pub async fn enrich_cart_delivery_groups_typed(',
  'pub async fn enrich_cart_delivery_groups(',
  'typed shipping enrichment',
);
const compatibilityWrapper = between(
  shipping,
  'pub async fn enrich_cart_delivery_groups(',
  'fn log_cart_delivery_group_enrichment_error(',
  'shipping compatibility wrapper',
);
const diagnostic = between(
  shipping,
  'fn log_cart_delivery_group_enrichment_error(',
  'fn extract_allowed_shipping_profile_slugs_from_metadata(',
  'shipping enrichment diagnostic',
);

for (const [value, label] of [
  ['db: &DatabaseConnection', 'database argument'],
  ['tenant_id: Uuid', 'tenant argument'],
  ['cart: CartResponse', 'cart argument'],
  ['public_channel_slug: Option<&str>', 'channel argument'],
  ['requested_locale: Option<&str>', 'requested locale argument'],
  ['tenant_default_locale: Option<&str>', 'default locale argument'],
  ['-> CommerceResult<CartResponse>', 'compatibility return type'],
  ['let cart_id = cart.id;', 'cart identity capture'],
  ['enrich_cart_delivery_groups_typed(', 'typed enrichment delegation'],
  [
    `enrich_cart_delivery_groups_typed(
        db,
        tenant_id,
        cart,
        public_channel_slug,
        requested_locale,
        tenant_default_locale,
    )
    .await`,
    'exact typed delegation arguments',
  ],
  ['.map_err(|error| {', 'compatibility error boundary'],
  ['log_cart_delivery_group_enrichment_error(', 'bounded diagnostic call'],
  [
    `log_cart_delivery_group_enrichment_error(
            &error,
            tenant_id,
            cart_id,
            public_channel_slug,
            requested_locale,
            tenant_default_locale,
        );`,
    'exact diagnostic arguments',
  ],
  [
    `crate::CommerceError::Validation(
            "Cart shipping details are temporarily unavailable".to_string(),
        )`,
    'stable public validation envelope',
  ],
]) {
  requireText(compatibilityWrapper, value, label);
}

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'format!("{error:?}")',
  'format!("{error}")',
  'CommerceError::Database',
]) {
  forbidText(compatibilityWrapper, value, 'unsafe compatibility public conversion');
}

const typedCallCount =
  compatibilityWrapper.match(/enrich_cart_delivery_groups_typed\(/g)?.length ?? 0;
if (typedCallCount !== 1) {
  failures.push(`typed compatibility call count: expected 1, found ${typedCallCount}`);
}
const diagnosticCallCount =
  compatibilityWrapper.match(/log_cart_delivery_group_enrichment_error\(/g)?.length ?? 0;
if (diagnosticCallCount !== 1) {
  failures.push(`compatibility diagnostic call count: expected 1, found ${diagnosticCallCount}`);
}
const validationEnvelopeCount =
  compatibilityWrapper.match(/crate::CommerceError::Validation\(/g)?.length ?? 0;
if (validationEnvelopeCount !== 1) {
  failures.push(`stable validation envelope count: expected 1, found ${validationEnvelopeCount}`);
}

const diagnosticIndex = compatibilityWrapper.indexOf(
  'log_cart_delivery_group_enrichment_error(',
);
const publicIndex = compatibilityWrapper.indexOf('crate::CommerceError::Validation(');
if (!(diagnosticIndex >= 0 && diagnosticIndex < publicIndex)) {
  failures.push('bounded diagnostic must run before the stable public mapping');
}

for (const [value, label] of [
  ['-> FulfillmentResult<CartResponse>', 'typed owner result'],
  ['FulfillmentService::new(db.clone())', 'fulfillment service construction'],
  [
    '.list_shipping_options(tenant_id, requested_locale, tenant_default_locale)',
    'typed owner call and arguments',
  ],
  ['.await?;', 'typed owner error propagation'],
  ['enrich_cart_delivery_groups_from_options(', 'pure option projection'],
]) {
  requireText(typedEnrichment, value, label);
}
for (const value of [
  'CommerceError::Validation',
  'error.to_string()',
  'err.to_string()',
]) {
  forbidText(typedEnrichment, value, 'typed enrichment error erasure');
}

for (const [value, label] of [
  ['struct StorefrontShippingDiagnosticError;', 'redacted diagnostic token'],
  ['formatter.write_str("redacted")', 'redacted diagnostic output'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option classification'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment classification'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['tenant_id_shape,', 'bounded tenant field'],
  ['cart_id_shape,', 'bounded cart field'],
  ['public_channel_slug_shape,', 'bounded channel field'],
  ['requested_locale_shape,', 'bounded locale field'],
  ['tenant_default_locale_shape,', 'bounded default-locale field'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['operation = "list_shipping_options"', 'owner operation'],
  ['boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY', 'diagnostic boundary'],
]) {
  requireText(`${shipping}\n${diagnostic}`, value, label);
}
for (const value of [
  'tenant_id = %tenant_id',
  'cart_id = %cart_id',
  'public_channel_slug = ?public_channel_slug',
  'requested_locale = ?requested_locale',
  'tenant_default_locale = ?tenant_default_locale',
]) {
  forbidText(diagnostic, value, 'raw diagnostic context');
}

const compatibilityDefinitions =
  shipping.match(/pub async fn enrich_cart_delivery_groups\(/g)?.length ?? 0;
if (compatibilityDefinitions !== 1) {
  failures.push(
    `compatibility definition count: expected 1, found ${compatibilityDefinitions}`,
  );
}
const typedDefinitions =
  shipping.match(/pub async fn enrich_cart_delivery_groups_typed\(/g)?.length ?? 0;
if (typedDefinitions !== 1) {
  failures.push(`typed definition count: expected 1, found ${typedDefinitions}`);
}

if (failures.length > 0) {
  console.error('Commerce storefront shipping public envelope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce storefront shipping compatibility mapping logs bounded owner facts before returning one stable public envelope',
);
