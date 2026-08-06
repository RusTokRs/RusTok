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
const ownerSource = read('crates/rustok-commerce/src/services/context.rs');
const resolverSource = read('crates/rustok-commerce/src/graphql/mutations/cart.rs');
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

const boundary = between(
  facade,
  'mod cart_context_boundary {',
  'mod cart_storefront_owner_boundary {',
  'cart store-context diagnostic boundary',
);

for (const [value, label] of [
  ['struct StoreContextDiagnosticError;', 'zero-sized diagnostic token'],
  ['impl std::fmt::Debug for StoreContextDiagnosticError', 'custom diagnostic Debug'],
  ['formatter.write_str("redacted")', 'redacted diagnostic output'],
  ['fn store_context_error_envelope(', 'typed public mapper'],
  ['impl From<StoreContextError> for BoundaryError', 'store-context conversion'],
  [
    'let (message, code, retryable, error_kind) = store_context_error_envelope(&error);',
    'typed envelope selection',
  ],
  ['let error = StoreContextDiagnosticError;', 'fail-closed diagnostic shadow'],
  ['tracing::error!(', 'single diagnostic event'],
  ['error = ?error', 'redacted diagnostic field'],
  ['owner = "rustok_commerce.store_context"', 'owner field'],
  ['error_kind,', 'typed owner kind'],
  ['public_code = code', 'public code field'],
  ['retryable,', 'retryability field'],
  ['operation = "resolve_store_context"', 'operation field'],
  ['boundary = "commerce_graphql_cart"', 'boundary field'],
  [
    '"commerce GraphQL cart store context resolution failed"',
    'static diagnostic message',
  ],
  ['Self::Public {', 'public envelope construction'],
  ['message,', 'public message field'],
  ['code,', 'public code envelope field'],
  ['retryable,', 'public retryability envelope field'],
  ['BoundaryError::Graphql(error) => error', 'GraphQL pass-through preservation'],
  [
    '} => public_graphql_error(message, code, retryable)',
    'public GraphQL restoration',
  ],
]) {
  requireText(boundary, value, label);
}

for (const [value, label] of [
  ['StoreContextError::TenantNotFound(_)', 'tenant not-found policy'],
  ['StoreContextError::Validation(_)', 'validation policy'],
  ['StoreContextError::CurrencyRegionMismatch { .. }', 'currency-region policy'],
  ['StoreContextError::TenantBoundary { .. }', 'tenant boundary policy'],
  ['StoreContextError::RegionBoundary { .. }', 'region boundary policy'],
  ['StoreContextError::Database(_)', 'database policy'],
  ['"STORE_CONTEXT_NOT_FOUND"', 'not-found code'],
  ['"STORE_CONTEXT_REQUEST_INVALID"', 'request-invalid code'],
  ['"STORE_CONTEXT_RESOLUTION_FAILED"', 'resolution-failed code'],
  ['"STORE_CONTEXT_TEMPORARILY_UNAVAILABLE"', 'temporary code'],
  ['"tenant_not_found"', 'tenant error kind'],
  ['"validation"', 'validation error kind'],
  ['"tenant_boundary"', 'tenant-boundary error kind'],
  ['"region_boundary"', 'region-boundary error kind'],
  ['"database"', 'database error kind'],
]) {
  requireText(boundary, value, label);
}

for (const value of [
  '#[derive(Debug)]\n    struct StoreContextDiagnosticError',
  '#[derive(Debug,',
  'async_graphql::Error::new(error.to_string())',
  'Error::new(error.to_string())',
  'format!("{error}")',
  'format!("{:?}", error)',
  'internal_message =',
  'database_error =',
  'validation_message =',
  'boundary_message =',
  'tenant_id =',
  'region_id =',
  'currency_code =',
  'region_currency_code =',
]) {
  forbidText(boundary, value, 'raw store-context diagnostic');
}

const envelopeIndex = boundary.indexOf(
  'let (message, code, retryable, error_kind) = store_context_error_envelope(&error);',
);
const shadowIndex = boundary.indexOf('let error = StoreContextDiagnosticError;');
const eventIndex = boundary.indexOf('tracing::error!(', shadowIndex);
const publicIndex = boundary.indexOf('Self::Public {', eventIndex);
if (
  !(
    envelopeIndex >= 0 &&
    envelopeIndex < shadowIndex &&
    shadowIndex < eventIndex &&
    eventIndex < publicIndex
  )
) {
  failures.push('store-context error must map policy, shadow payload, emit, and return in order');
}

for (const [pattern, expected, label] of [
  [/struct StoreContextDiagnosticError;/g, 1, 'diagnostic token count'],
  [/let error = StoreContextDiagnosticError;/g, 1, 'diagnostic shadow count'],
  [/tracing::error!\(/g, 1, 'diagnostic event count'],
  [/"STORE_CONTEXT_TEMPORARILY_UNAVAILABLE"/g, 1, 'retryable policy count'],
]) {
  const count = boundary.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['TenantNotFound(Uuid)', 'owner tenant payload variant'],
  ['Validation(String)', 'owner validation payload variant'],
  ['CurrencyRegionMismatch {', 'owner currency payload variant'],
  ['TenantBoundary { code: String, message: String }', 'tenant boundary payload variant'],
  ['RegionBoundary { code: String, message: String }', 'region boundary payload variant'],
  ['Database(#[from] sea_orm::DbErr)', 'database payload variant'],
]) {
  requireText(ownerSource, value, label);
}

for (const [pattern, expected, label] of [
  [/StoreContextService::new\(/g, 2, 'store-context service constructor count'],
  [/\.resolve_context\(/g, 2, 'store-context resolution call count'],
]) {
  const count = resolverSource.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart context diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL cart store-context diagnostics expose only typed policy and a redacted error token while public envelopes remain unchanged',
);
