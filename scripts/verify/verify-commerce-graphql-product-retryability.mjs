#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const graphqlRoot = read('crates/rustok-commerce/src/graphql/mod.rs');
const catalogMutations = read('crates/rustok-commerce/src/graphql/mutations/catalog.rs');
const queries = read('crates/rustok-commerce/src/graphql/query.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['pub(crate) fn map_product_service_error(', 'shared product mapper'],
  ['let (public_message, code, retryable) = match &error {', 'typed public envelope'],
  ['CommerceError::Database(_) => (', 'database mapping'],
  ['"PRODUCT_TEMPORARILY_UNAVAILABLE"', 'temporary-unavailable code'],
  ['CommerceError::ProductNotFound(_)', 'product not-found mapping'],
  ['"PRODUCT_NOT_FOUND"', 'product not-found code'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found mapping'],
  ['"VARIANT_NOT_FOUND"', 'variant not-found code'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate-handle mapping'],
  ['"DUPLICATE_HANDLE"', 'duplicate-handle code'],
  ['CommerceError::DuplicateSku(_)', 'duplicate-SKU mapping'],
  ['"DUPLICATE_SKU"', 'duplicate-SKU code'],
  ['CommerceError::InvalidPrice(_)', 'invalid-price mapping'],
  ['"INVALID_PRICE"', 'invalid-price code'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory mapping'],
  ['"INSUFFICIENT_INVENTORY"', 'inventory code'],
  ['CommerceError::InvalidOptionCombination', 'option mapping'],
  ['"INVALID_OPTIONS"', 'option code'],
  ['CommerceError::Validation(_)', 'validation mapping'],
  ['"PRODUCT_VALIDATION"', 'validation code'],
  ['CommerceError::ShippingProfileNotFound(_)', 'shipping-profile mapping'],
  ['"SHIPPING_PROFILE_NOT_FOUND"', 'shipping-profile code'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'shipping-profile conflict mapping'],
  ['"DUPLICATE_SHIPPING_PROFILE_SLUG"', 'shipping-profile conflict code'],
  ['CommerceError::NoVariants', 'no-variants mapping'],
  ['"NO_VARIANTS"', 'no-variants code'],
  ['CommerceError::CannotDeletePublished', 'published-delete mapping'],
  ['"CANNOT_DELETE_PUBLISHED"', 'published-delete code'],
  ['CommerceError::Rich(_) | CommerceError::Core(_)', 'safe fallback mapping'],
  ['"PRODUCT_OPERATION_FAILED"', 'safe fallback code'],
  ['public_code = code', 'public code logging'],
  ['retryable,', 'retryability logging'],
  ['boundary = "commerce_graphql_product"', 'boundary logging'],
  ['extensions.set("code", code)', 'code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
]) {
  requireText(graphqlRoot, value, label);
}

forbidText(
  graphqlRoot,
  'let (public_message, code) = match error {',
  'legacy code-only product envelope',
);

const databaseBlock = graphqlRoot.match(
  /CommerceError::Database\(_\) => \([\s\S]*?"PRODUCT_TEMPORARILY_UNAVAILABLE",\s*true,\s*\)/,
);
if (!databaseBlock) {
  failures.push('database mapping must be the retryable PRODUCT_TEMPORARILY_UNAVAILABLE envelope');
}

const retryableTrueOccurrences = graphqlRoot.match(/\btrue,\s*\)/g) ?? [];
if (retryableTrueOccurrences.length !== 1) {
  failures.push(`expected exactly one retryable product envelope, found ${retryableTrueOccurrences.length}`);
}

const retryableExtensionOccurrences =
  graphqlRoot.match(/extensions\.set\("retryable", retryable\)/g) ?? [];
if (retryableExtensionOccurrences.length !== 1) {
  failures.push(
    `expected one product retryability extension, found ${retryableExtensionOccurrences.length}`,
  );
}

for (const [content, value, label] of [
  [catalogMutations, 'map_product_service_error(error, "product_catalog_mutation")', 'catalog mutation mapper use'],
  [queries, 'map_product_service_error(err, "product_query")', 'product query mapper use'],
  [queries, 'map_product_service_error(err, "products_query")', 'products query mapper use'],
]) {
  requireText(content, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL product retryability verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL product service errors expose stable codes and retryability across queries and mutations',
);
