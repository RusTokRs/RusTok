#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_cart.rs');
const source = read('crates/rustok-commerce/src/graphql/mutations/cart.rs');
const ownerSource = read('crates/rustok-commerce/src/services/context.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(
  routing,
  '#[path = "safe_cart.rs"]\npub mod cart;',
  'cart safe module routing',
);
const cartModuleDeclarations = routing.match(/pub mod cart;/g) ?? [];
if (cartModuleDeclarations.length !== 1) {
  failures.push(`expected one cart module declaration, found ${cartModuleDeclarations.length}`);
}

for (const [value, label] of [
  ['mod cart_context_boundary {', 'cart context boundary module'],
  ['#[derive(Clone)]', 'async GraphQL clone requirement'],
  ['pub(crate) enum BoundaryError {', 'local boundary error'],
  ['Graphql(Error)', 'GraphQL pass-through variant'],
  ['Public {', 'cloneable public envelope variant'],
  ['impl From<Error> for BoundaryError', 'GraphQL conversion'],
  ['impl From<StoreContextError> for BoundaryError', 'store context conversion'],
  ['impl From<BoundaryError> for Error', 'public GraphQL conversion'],
  ['BoundaryError::Graphql(error) => error', 'existing GraphQL error preservation'],
  ['BoundaryError::Public {', 'public envelope restoration'],
  ['fn store_context_error_envelope(', 'store context mapper'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['owner = "rustok_commerce.store_context"', 'owner logging'],
  ['error_kind,', 'owner error kind logging'],
  ['public_code = code', 'public code logging'],
  ['operation = "resolve_store_context"', 'operation logging'],
  ['boundary = "commerce_graphql_cart"', 'cart boundary logging'],
  ['mod async_graphql_shim {', 'async GraphQL result shim'],
  [
    'pub use ::async_graphql::{Context, Error, MaybeUndefined, Object};',
    'GraphQL API re-export',
  ],
  [
    'pub type Result<T> = std::result::Result<T, super::cart_context_boundary::BoundaryError>;',
    'custom cart result',
  ],
  ['use self::async_graphql_shim as async_graphql;', 'shim alias'],
  ['include!("cart.rs");', 'unchanged cart resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const value of [
  'StoreContext(StoreContextError)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(facade, value, 'cart context public boundary');
}
for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(source, value, 'cart resolver public boundary');
}

for (const [value, label] of [
  ['StoreContextError::TenantNotFound(_)', 'tenant not-found mapping'],
  ['StoreContextError::Validation(_)', 'validation mapping'],
  ['StoreContextError::CurrencyRegionMismatch { .. }', 'currency-region mapping'],
  ['StoreContextError::RegionBoundary { .. }', 'region boundary mapping'],
  ['StoreContextError::Database(_)', 'database mapping'],
  ['"STORE_CONTEXT_NOT_FOUND"', 'not-found code'],
  ['"STORE_CONTEXT_REQUEST_INVALID"', 'validation code'],
  ['"STORE_CONTEXT_RESOLUTION_FAILED"', 'region boundary code'],
  ['"STORE_CONTEXT_TEMPORARILY_UNAVAILABLE"', 'temporary code'],
  ['"tenant_not_found"', 'tenant error kind'],
  ['"validation"', 'validation error kind'],
  ['"region_boundary"', 'region boundary error kind'],
  ['"database"', 'database error kind'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['TenantNotFound(Uuid)', 'owner tenant variant'],
  ['Validation(String)', 'owner validation variant'],
  ['CurrencyRegionMismatch {', 'owner currency-region variant'],
  ['RegionBoundary { code: String, message: String }', 'owner region boundary variant'],
  ['Database(#[from] sea_orm::DbErr)', 'owner database variant'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['use async_graphql::{Context, Object, Result};', 'resolver Result import'],
  ['use crate::StoreContextService;', 'store context service import'],
  ['async fn create_storefront_cart(', 'create cart mutation'],
  ['async fn update_storefront_cart_context(', 'update context mutation'],
]) {
  requireText(source, value, label);
}

const serviceConstructors = source.match(/StoreContextService::new\(/g) ?? [];
if (serviceConstructors.length !== 2) {
  failures.push(`expected two store-context service constructors, found ${serviceConstructors.length}`);
}
const contextResolutions = source.match(/\.resolve_context\(/g) ?? [];
if (contextResolutions.length !== 2) {
  failures.push(`expected two store-context resolution call sites, found ${contextResolutions.length}`);
}

const temporaryCodeOccurrences = facade.match(/"STORE_CONTEXT_TEMPORARILY_UNAVAILABLE"/g) ?? [];
if (temporaryCodeOccurrences.length !== 1) {
  failures.push(`expected one retryable database envelope, found ${temporaryCodeOccurrences.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart context error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL cart context resolution uses cloneable typed public envelopes while preserving existing GraphQL errors',
);
