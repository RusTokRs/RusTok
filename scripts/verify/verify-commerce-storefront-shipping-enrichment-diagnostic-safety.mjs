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
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
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
  [
    'const STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY: &str = "commerce_storefront_shipping_enrichment";',
    'shipping diagnostic boundary',
  ],
  ['struct StorefrontShippingDiagnosticError;', 'redacted diagnostic token'],
  [
    'impl std::fmt::Debug for StorefrontShippingDiagnosticError',
    'diagnostic token Debug implementation',
  ],
  ['formatter.write_str("redacted")', 'diagnostic token redaction'],
  ['fn uuid_shape(value: Uuid)', 'UUID shape helper'],
  ['if value.is_nil() { "nil" } else { "non_nil" }', 'closed UUID shape'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
  ['None => "absent"', 'absent text shape'],
  ['Some(value) if value.is_empty() => "empty"', 'empty text shape'],
  ['Some(_) => "present"', 'present text shape'],
]) {
  requireText(shipping, value, label);
}

for (const [value, label] of [
  ['error: &FulfillmentError', 'typed owner error input'],
  ['tenant_id: Uuid', 'tenant identity input'],
  ['cart_id: Uuid', 'cart identity input'],
  ['public_channel_slug: Option<&str>', 'channel input'],
  ['requested_locale: Option<&str>', 'requested locale input'],
  ['tenant_default_locale: Option<&str>', 'default locale input'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option classification'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment classification'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['"fulfillment.validation"', 'validation owner code'],
  ['"fulfillment.shipping_option_not_found"', 'shipping-option owner code'],
  ['"fulfillment.fulfillment_not_found"', 'fulfillment owner code'],
  ['"fulfillment.invalid_transition"', 'transition owner code'],
  ['"fulfillment.database_unavailable"', 'database owner code'],
  ['let technical = matches!(error, FulfillmentError::Database(_));', 'technical severity fact'],
  ['let tenant_id_shape = uuid_shape(tenant_id);', 'tenant shape projection'],
  ['let cart_id_shape = uuid_shape(cart_id);', 'cart shape projection'],
  [
    'let public_channel_slug_shape = optional_text_shape(public_channel_slug);',
    'channel shape projection',
  ],
  ['let requested_locale_shape = optional_text_shape(requested_locale);', 'locale shape projection'],
  [
    'let tenant_default_locale_shape = optional_text_shape(tenant_default_locale);',
    'default-locale shape projection',
  ],
  ['let error = StorefrontShippingDiagnosticError;', 'redacted source shadow'],
  ['if technical {', 'technical event branch'],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['error = ?error', 'redacted error field'],
  ['owner = "rustok_fulfillment"', 'truthful owner field'],
  ['tenant_id_shape,', 'tenant shape field'],
  ['cart_id_shape,', 'cart shape field'],
  ['public_channel_slug_shape,', 'channel shape field'],
  ['requested_locale_shape,', 'requested locale shape field'],
  ['tenant_default_locale_shape,', 'default locale shape field'],
  ['operation = "list_shipping_options"', 'owner operation field'],
  ['owner_code,', 'owner code field'],
  ['owner_kind,', 'owner kind field'],
  ['owner_retryable,', 'owner retryability field'],
  ['boundary = STOREFRONT_SHIPPING_ENRICHMENT_BOUNDARY', 'boundary field'],
  [
    '"storefront cart shipping enrichment owner read failed"',
    'technical static event message',
  ],
  [
    '"storefront cart shipping enrichment owner read was rejected"',
    'ordinary static event message',
  ],
]) {
  requireText(diagnostic, value, label);
}

for (const value of [
  'tenant_id = %tenant_id',
  'tenant_id = ?tenant_id',
  'cart_id = %cart_id',
  'cart_id = ?cart_id',
  'public_channel_slug = ?public_channel_slug',
  'public_channel_slug = %public_channel_slug',
  'requested_locale = ?requested_locale',
  'requested_locale = %requested_locale',
  'tenant_default_locale = ?tenant_default_locale',
  'tenant_default_locale = %tenant_default_locale',
  'error.message',
  'error.to_string()',
  'format!("{error:?}")',
]) {
  forbidText(diagnostic, value, 'raw shipping enrichment diagnostic');
}

const classificationIndex = diagnostic.indexOf(
  'let (owner_code, owner_kind, owner_retryable) = match error',
);
const severityIndex = diagnostic.indexOf(
  'let technical = matches!(error, FulfillmentError::Database(_));',
);
const projectionIndex = diagnostic.indexOf('let tenant_id_shape = uuid_shape(tenant_id);');
const shadowIndex = diagnostic.indexOf('let error = StorefrontShippingDiagnosticError;');
const eventIndex = diagnostic.indexOf('tracing::error!(');
if (
  !(
    classificationIndex >= 0 &&
    classificationIndex < severityIndex &&
    severityIndex < projectionIndex &&
    projectionIndex < shadowIndex &&
    shadowIndex < eventIndex
  )
) {
  failures.push(
    'shipping enrichment error must classify, select severity, project, shadow, and diagnose in order',
  );
}

for (const [pattern, expected, label] of [
  [/let error = StorefrontShippingDiagnosticError;/g, 1, 'redacted shadow count'],
  [/error = \?error/g, 2, 'redacted error field count'],
  [/tracing::error!\(/g, 1, 'technical event count'],
  [/tracing::warn!\(/g, 1, 'ordinary event count'],
  [/tenant_id_shape,/g, 2, 'tenant shape field count'],
  [/cart_id_shape,/g, 2, 'cart shape field count'],
  [/public_channel_slug_shape,/g, 2, 'channel shape field count'],
  [/requested_locale_shape,/g, 2, 'requested locale shape field count'],
  [/tenant_default_locale_shape,/g, 2, 'default locale shape field count'],
]) {
  const count = diagnostic.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['-> FulfillmentResult<CartResponse>', 'typed enrichment result'],
  ['FulfillmentService::new(db.clone())', 'fulfillment service construction'],
  [
    '.list_shipping_options(tenant_id, requested_locale, tenant_default_locale)',
    'typed owner call and arguments',
  ],
  ['.await?;', 'typed owner error propagation'],
  ['enrich_cart_delivery_groups_from_options(', 'unchanged option projection'],
]) {
  requireText(typedEnrichment, value, label);
}

for (const [value, label] of [
  ['-> CommerceResult<CartResponse>', 'legacy compatibility result'],
  ['enrich_cart_delivery_groups_typed(', 'typed implementation delegation'],
  ['log_cart_delivery_group_enrichment_error(', 'diagnostic call'],
  [
    'crate::CommerceError::Validation(\n            "Cart shipping details are temporarily unavailable".to_string(),\n        )',
    'stable compatibility public envelope',
  ],
]) {
  requireText(compatibilityWrapper, value, label);
}
for (const value of ['error.to_string()', 'err.to_string()', 'format!("{error:?}")']) {
  forbidText(compatibilityWrapper, value, 'unsafe compatibility public conversion');
}

for (const [value, label] of [
  ['pub enum FulfillmentError {', 'owner error enum'],
  ['Validation(String)', 'owner validation payload'],
  ['ShippingOptionNotFound(Uuid)', 'owner shipping option payload'],
  ['FulfillmentNotFound(Uuid)', 'owner fulfillment payload'],
  ['InvalidTransition { from: String, to: String }', 'owner transition payload'],
  ['Database(#[from] DbErr)', 'owner database payload'],
]) {
  requireText(fulfillmentErrors, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront shipping enrichment diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce storefront shipping enrichment diagnostics keep typed owner facts, redact raw request context, and return a stable compatibility envelope',
);
