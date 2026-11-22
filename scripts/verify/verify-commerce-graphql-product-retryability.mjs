#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const graphqlRoot = read('crates/modules/rustok-commerce/src/graphql/mod.rs');
const catalogMutations = read('crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs');
const productCatalogQueries = read('crates/modules/rustok-commerce/src/graphql/product_catalog.rs');
const publicError = read('crates/modules/rustok-product/src/public_error.rs');

const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

// 1. Shared GraphQL-side product mapper must delegate to the owner public-error envelope.
const mapperStart = graphqlRoot.indexOf('pub(crate) fn map_product_service_error(');
const mapperEnd = graphqlRoot.indexOf('\npub(crate) fn current_tenant_scope(', mapperStart);
if (mapperStart < 0 || mapperEnd < 0) {
  failures.push('unable to isolate shared product mapper');
}
const mapper = mapperStart >= 0 && mapperEnd >= 0
  ? graphqlRoot.slice(mapperStart, mapperEnd)
  : graphqlRoot;

for (const [value, label] of [
  ['pub(crate) fn map_product_service_error(', 'shared product mapper'],
  [
    'rustok_product::map_product_public_error(&error, operation, "commerce_graphql_product")',
    'owner public-error delegation with commerce boundary',
  ],
  ['extensions.set("code", public.code)', 'code extension'],
  ['extensions.set("retryable", public.retryable)', 'retryability extension'],
  [
    'extensions.set("correlation_id", public.correlation_id.to_string())',
    'correlation extension',
  ],
]) {
  requireText(mapper, value, label);
}

forbidText(
  mapper,
  'let (public_message, code) = match error {',
  'legacy code-only product envelope',
);

// 2. Owner typed envelope: stable codes plus retryability, with bounded logging.
for (const [value, label] of [
  ['pub fn map_product_public_error(', 'owner public-error mapper'],
  ['let (message, code, retryable) = match error {', 'typed public envelope'],
  ['CommerceError::Database(_) => (', 'database mapping'],
  ['"PRODUCT_TEMPORARILY_UNAVAILABLE"', 'temporary-unavailable code'],
  ['CommerceError::ProductNotFound(_)', 'product not-found mapping'],
  ['"PRODUCT_NOT_FOUND"', 'product not-found code'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found mapping'],
  ['"PRODUCT_VARIANT_NOT_FOUND"', 'variant not-found code'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate-handle mapping'],
  ['"DUPLICATE_HANDLE"', 'duplicate-handle code'],
  ['CommerceError::DuplicateSku(_)', 'duplicate-SKU mapping'],
  ['"DUPLICATE_SKU"', 'duplicate-SKU code'],
  ['CommerceError::Validation(_)', 'validation mapping'],
  ['"PRODUCT_VALIDATION"', 'validation code'],
  ['CommerceError::NoVariants', 'no-variants mapping'],
  ['"NO_VARIANTS"', 'no-variants code'],
  ['CommerceError::CannotDeletePublished', 'published-delete mapping'],
  ['"CANNOT_DELETE_PUBLISHED"', 'published-delete code'],
  ['CommerceError::InventoryQuantityBlocked(blocker) =>', 'inventory mapping'],
  ['crate::error::inventory_quantity_public_policy(blocker)', 'inventory policy mapping'],
  [
    'CommerceError::Conflict(_) | CommerceError::VariantAxisPolicyConflict(_) =>',
    'conflict and axis-policy mapping',
  ],
  ['"PRODUCT_CONFLICT"', 'conflict code'],
  ['CommerceError::Core(error) => core_public_error_policy(error)', 'safe fallback mapping'],
  ['"PRODUCT_OPERATION_FAILED"', 'safe fallback code'],
  ['public_code = code', 'public code logging'],
  ['retryable,', 'retryability logging'],
  ['boundary,', 'boundary logging'],
]) {
  requireText(publicError, value, label);
}

const databaseBlock = publicError.match(
  /CommerceError::Database\(_\) => \([\s\S]*?"PRODUCT_TEMPORARILY_UNAVAILABLE",\s*true,\s*\)/,
);
if (!databaseBlock) {
  failures.push('database mapping must be the retryable PRODUCT_TEMPORARILY_UNAVAILABLE envelope');
}

const retryableTrueOccurrences = publicError.match(/\btrue,\s*\)/g) ?? [];
if (retryableTrueOccurrences.length !== 1) {
  failures.push(`expected exactly one retryable product envelope, found ${retryableTrueOccurrences.length}`);
}

const retryableExtensionOccurrences =
  mapper.match(/extensions\.set\("retryable", public\.retryable\)/g) ?? [];
if (retryableExtensionOccurrences.length !== 1) {
  failures.push(
    `expected one product retryability extension, found ${retryableExtensionOccurrences.length}`,
  );
}

// 3. Mutations map owner PortErrors through the command-port envelope with terminality.
for (const [value, label] of [
  [
    'product_command_port_error(&port_context, error, "create_product")',
    'catalog mutation mapper use',
  ],
  [
    'rustok_product::product_catalog_command_error_is_terminal(error)',
    'command terminality classification',
  ],
  ['retryable = error.retryable', 'command retryability logging'],
  ['public_code = code', 'command public code logging'],
]) {
  requireText(catalogMutations, value, label);
}

// 4. Storefront/admin catalog queries and facets reuse the shared mapper.
for (const [value, label] of [
  [
    'map_product_service_error(error, "storefront_product_catalog_input")',
    'storefront catalog mapper use',
  ],
  [
    'map_product_service_error(error, "admin_product_catalog_input")',
    'admin catalog mapper use',
  ],
]) {
  requireText(productCatalogQueries, value, label);
}

const catalogMapperUses =
  catalogMutations.match(/product_command_port_error\(&port_context/g) ?? [];
if (catalogMapperUses.length < 5) {
  failures.push(`expected shared mapper across catalog mutations, found ${catalogMapperUses.length} uses`);
}
const queryMapperUses = productCatalogQueries.match(/map_product_service_error\(/g) ?? [];
if (queryMapperUses.length < 5) {
  failures.push(`expected shared mapper across product queries, found ${queryMapperUses.length} uses`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL product retryability verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL product service errors expose stable codes and retryability across queries and mutations',
);
