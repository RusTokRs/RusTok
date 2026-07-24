#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-product/src/ports.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  '.map_err(product_error_to_port_error)',
  'fn product_error_to_port_error(\n    error:',
  '"PortContext.tenant_id must be a UUID for product ports"',
  'format!("product storage unavailable: {error}")',
  'format!("variant {} not found", request.variant_id)',
  'format!("product {id} not found")',
  'format!("duplicate handle `{handle}` for locale `{locale}`")',
  'PortError::validation("product.validation", message)',
  'format!("product operation failed: {other}")',
]) {
  forbidText(value, 'product catalog public error mapping');
}

for (const [value, label] of [
  [
    'const READ_PRODUCT_PROJECTION_OPERATION',
    'product projection owner operation',
  ],
  [
    'const READ_VARIANT_PRODUCT_PROJECTION_OPERATION',
    'variant projection owner operation',
  ],
  [
    'const LIST_PUBLISHED_PRODUCTS_OPERATION',
    'published product list owner operation',
  ],
  [
    'let owner_operation = READ_PRODUCT_PROJECTION_OPERATION;',
    'product projection operation mapping',
  ],
  [
    'let owner_operation = READ_VARIANT_PRODUCT_PROJECTION_OPERATION;',
    'variant projection operation mapping',
  ],
  [
    'let owner_operation = LIST_PUBLISHED_PRODUCTS_OPERATION;',
    'published product list operation mapping',
  ],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation = owner_operation', 'owner operation logging'],
  ['code = "product.context_invalid"', 'context stable code'],
  ['code = "product.database_unavailable"', 'database stable code'],
  ['code = "product.variant_not_found"', 'variant stable code'],
  ['"product request context is invalid"', 'stable context message'],
  ['"product storage is temporarily unavailable"', 'stable storage message'],
  ['"product variant was not found"', 'stable variant message'],
  ['"product was not found"', 'stable product message'],
  ['"product request is invalid"', 'stable validation message'],
  [
    '"product handle conflicts with an existing product"',
    'stable duplicate-handle message',
  ],
  [
    '"product operation could not be completed safely"',
    'stable invariant message',
  ],
  [
    'product_error_to_port_error(&context, owner_operation, error)',
    'context-aware owner mapping',
  ],
  [
    'product_storage_error(&context, owner_operation, error)',
    'context-aware storage mapping',
  ],
  [
    'product_variant_not_found(&context, owner_operation, request.variant_id)',
    'context-aware variant mapping',
  ],
  [
    'parse_port_tenant_id(&context, owner_operation)',
    'context-aware tenant mapping',
  ],
  [
    'validate_published_products_request(&context, owner_operation, &request)',
    'context-aware pagination validation',
  ],
]) {
  requireText(value, label);
}

if (failures.length > 0) {
  console.error('Product catalog port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Product catalog reads keep technical and identifying owner failures in correlation-aware logs and expose stable public errors',
);
