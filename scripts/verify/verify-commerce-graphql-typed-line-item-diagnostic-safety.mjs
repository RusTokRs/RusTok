#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_line_item_helpers.rs',
);
const layeredSource = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
);
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
  source,
  'fn storefront_line_item_graphql_error(',
  'fn parse_line_item_metadata(',
  'typed line-item GraphQL mapper',
);
const publicPolicy = between(
  source,
  'fn storefront_line_item_public_policy(',
  'fn uuid_shape(',
  'typed line-item public policy',
);

for (const [value, label] of [
  ['enum StorefrontLineItemFailureKind {', 'typed failure kind'],
  ['ProductUnavailable,', 'product-unavailable outcome'],
  ['InventoryInsufficient,', 'inventory-insufficient outcome'],
  ['InputInvalid,', 'input-invalid outcome'],
  ['DependencyUnavailable,', 'dependency-unavailable outcome'],
  ['enum StorefrontLineItemFailureSource {', 'typed source enum'],
  ['Database(sea_orm::DbErr)', 'database source'],
  ['Pricing(PortError)', 'pricing source'],
  ['Inventory(CommerceError)', 'inventory source'],
  ['Metadata(serde_json::Error)', 'metadata source'],
  ["Local(&'static str)", 'local policy source'],
  ['fn kind(&self) -> &\'static str', 'source kind projection'],
  ['struct StorefrontLineItemDiagnosticSource;', 'zero-sized source token'],
  [
    'impl From<StorefrontLineItemFailureSource> for StorefrontLineItemDiagnosticSource',
    'source-consuming conversion',
  ],
  [
    'fn from(_source: StorefrontLineItemFailureSource) -> Self',
    'source payload consumption',
  ],
  ['impl std::fmt::Debug for StorefrontLineItemDiagnosticSource', 'custom source Debug'],
  ['formatter.write_str("redacted")', 'redacted source output'],
  ['fn uuid_shape(value: Uuid)', 'UUID shape helper'],
  ['fn optional_uuid_shape(value: Option<Uuid>)', 'optional UUID shape helper'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
  ['async fn resolve_typed_storefront_line_item_input(', 'typed resolver'],
  ['async fn validate_typed_storefront_line_item_quantity(', 'typed quantity validator'],
  ['async fn validate_typed_storefront_variant_inventory(', 'typed inventory validator'],
  ['pricing_context: &PriceResolutionContext', 'pricing context input'],
  ['.resolve_product_price(', 'pricing owner delegation'],
  ['region_id: pricing_context.region_id', 'pricing region delegation'],
  ['channel_id: pricing_context.channel_id', 'pricing channel delegation'],
  ['channel_slug: pricing_context.channel_slug.clone()', 'pricing channel slug delegation'],
  ['price_list_id: pricing_context.price_list_id', 'pricing list delegation'],
  ['quantity: pricing_context.quantity', 'pricing quantity delegation'],
  ['currency_code: pricing_context.currency_code.clone()', 'pricing currency delegation'],
  ['check_variant_availability_for_public_channel(', 'inventory owner delegation'],
  ['resolve_storefront_line_item_input(', 'mounted resolve helper'],
  ['validate_storefront_line_item_quantity(', 'mounted quantity helper'],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['"Product is not available"', 'product public message'],
  ['"CART_PRODUCT_UNAVAILABLE"', 'product public code'],
  ['"Requested quantity is not available"', 'inventory public message'],
  ['"CART_INVENTORY_INSUFFICIENT"', 'inventory public code'],
  ['"Cart line item input is invalid"', 'input public message'],
  ['"CART_LINE_ITEM_INVALID"', 'input public code'],
  ['"Cart line item could not be resolved"', 'resolve fallback message'],
  ['"CART_LINE_ITEM_RESOLUTION_FAILED"', 'resolve fallback code'],
  ['"Inventory availability could not be verified"', 'quantity fallback message'],
  ['"CART_INVENTORY_UNAVAILABLE"', 'quantity fallback code'],
]) {
  requireText(publicPolicy, value, label);
}

for (const [value, label] of [
  ['storefront_line_item_public_policy(consumer_operation, failure.kind)', 'typed policy selection'],
  ['let StorefrontLineItemFailure {', 'failure destructuring'],
  ['source_owner,', 'source owner extraction'],
  ['source_operation,', 'source operation extraction'],
  ['product_id,', 'product identity extraction'],
  ['let source_kind = source.kind();', 'source kind before consumption'],
  [
    'let source = StorefrontLineItemDiagnosticSource::from(source);',
    'source consumption and shadow',
  ],
  ['let correlation_id_shape = optional_text_shape(correlation_id);', 'correlation shape'],
  ['let tenant_id_shape = uuid_shape(tenant_id);', 'tenant UUID shape'],
  ['let variant_id_shape = uuid_shape(variant_id);', 'variant UUID shape'],
  ['let product_id_shape = optional_uuid_shape(product_id);', 'product UUID shape'],
  ['channel_slug_length = public_channel_slug.map', 'bounded channel length'],
  ['locale_length = locale.map', 'bounded locale length'],
  ['source = ?source', 'redacted source field'],
  ['source_kind,', 'source kind field'],
  ['owner = source_owner', 'source owner field'],
  ['owner_operation = source_operation', 'source operation field'],
  ['consumer_operation = consumer_operation.name()', 'consumer operation field'],
  ['failure_kind,', 'failure kind field'],
  ['correlation_id_shape,', 'correlation shape field'],
  ['tenant_id_shape,', 'tenant shape field'],
  ['variant_id_shape,', 'variant shape field'],
  ['product_id_shape,', 'product shape field'],
  ['requested_quantity,', 'quantity field'],
  ['channel_slug_length = ?channel_slug_length', 'channel length field'],
  ['locale_length = ?locale_length', 'locale length field'],
  ['public_code = code', 'public code field'],
  ['public_retryable = retryable', 'public retryability field'],
  ['boundary = STOREFRONT_LINE_ITEM_GRAPHQL_BOUNDARY', 'boundary field'],
  ['tracing::error!(', 'dependency error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"commerce GraphQL storefront line item dependency failed"', 'dependency event'],
  ['"commerce GraphQL storefront line item request was rejected"', 'rejection event'],
  ['public_graphql_error(message, code, retryable)', 'stable public envelope'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'source = ?failure.source',
  'source = ?failure.source.detail()',
  'let source = failure.source.detail()',
  '.detail()',
  'correlation_id = ?correlation_id',
  'correlation_id = %correlation_id',
  'tenant_id = %tenant_id',
  'tenant_id = ?tenant_id',
  'variant_id = %variant_id',
  'variant_id = ?variant_id',
  'product_id = ?failure.product_id',
  'product_id = %product_id',
  'public_channel_slug = ?public_channel_slug',
  'locale = ?locale',
  'metadata = ?input.metadata',
  'format!("{error:?}")',
  'error.to_string()',
  'error.message',
]) {
  forbidText(mapper, value, 'raw typed line-item diagnostic');
}
for (const value of ['use std::fmt::Debug;', 'fn detail(&self)', '-> &dyn Debug']) {
  forbidText(source, value, 'raw source rendering path');
}

const policyIndex = mapper.indexOf(
  'storefront_line_item_public_policy(consumer_operation, failure.kind)',
);
const destructureIndex = mapper.indexOf('let StorefrontLineItemFailure {');
const kindIndex = mapper.indexOf('let source_kind = source.kind();');
const shadowIndex = mapper.indexOf(
  'let source = StorefrontLineItemDiagnosticSource::from(source);',
);
const shapeIndex = mapper.indexOf(
  'let correlation_id_shape = optional_text_shape(correlation_id);',
);
const eventIndex = mapper.indexOf('tracing::error!(');
const returnIndex = mapper.lastIndexOf('public_graphql_error(message, code, retryable)');
if (
  !(
    policyIndex >= 0 &&
    policyIndex < destructureIndex &&
    destructureIndex < kindIndex &&
    kindIndex < shadowIndex &&
    shadowIndex < shapeIndex &&
    shapeIndex < eventIndex &&
    eventIndex < returnIndex
  )
) {
  failures.push(
    'typed failure must map policy, destructure, classify, consume, project, diagnose, and return in order',
  );
}

for (const [pattern, expected, label] of [
  [/StorefrontLineItemDiagnosticSource::from\(source\)/g, 1, 'source consumption count'],
  [/source = \?source/g, 2, 'redacted source field count'],
  [/tracing::error!\(/g, 1, 'dependency event count'],
  [/tracing::warn!\(/g, 1, 'rejection event count'],
  [/correlation_id_shape,/g, 2, 'correlation shape field count'],
  [/tenant_id_shape,/g, 2, 'tenant shape field count'],
  [/variant_id_shape,/g, 2, 'variant shape field count'],
  [/product_id_shape,/g, 2, 'product shape field count'],
]) {
  const count = mapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const operation of [
  'resolve_storefront_line_item_input',
  'validate_storefront_line_item_quantity',
]) {
  requireText(source, `"${operation}"`, `${operation} consumer operation`);
}
for (const [value, label] of [
  [
    'resolve_storefront_line_item_input, validate_storefront_line_item_quantity,',
    'two explicit typed exports',
  ],
]) {
  requireText(layeredSource, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL typed line-item diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL typed line-item diagnostics consume owner payloads, expose only closed identity shapes, and preserve typed public policy and owner delegations',
);
