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
const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_legacy_helpers.rs');
const helperSource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
const publicFacade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;', 'safe legacy helper routing'],
]) {
  requireText(routing, value, label);
}
for (const value of ['#[path = "helpers.rs"]\nmod legacy_helpers;']) {
  forbidText(routing, value, 'unsafe legacy helper routing');
}

for (const [value, label] of [
  ['mod rustok_fulfillment_shim {', 'fulfillment import shim'],
  ['pub struct FulfillmentService {', 'contextual fulfillment facade'],
  ['inner: ::rustok_fulfillment::FulfillmentService', 'canonical fulfillment owner field'],
  ['inner: ::rustok_fulfillment::FulfillmentService::new(db)', 'canonical fulfillment constructor'],
  ['pub async fn get_shipping_option(', 'shipping option interception'],
  ['log_shipping_option_error(', 'typed fulfillment cause logging'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['owner = "rustok_fulfillment"', 'truthful fulfillment owner'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['shipping_option_id = %shipping_option_id', 'shipping option identity'],
  ['requested_locale = ?requested_locale', 'requested locale context'],
  ['tenant_default_locale = ?tenant_default_locale', 'default locale context'],
  ['operation = "get_shipping_option"', 'exact owner operation'],
  ['owner_code,', 'stable owner code'],
  ['owner_kind,', 'typed owner kind'],
  ['owner_retryable,', 'owner retryability'],
  ['boundary = STOREFRONT_CART_LEGACY_HELPER_BOUNDARY', 'legacy helper boundary'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'fulfillment shim alias'],
  ['include!("helpers.rs");', 'unchanged legacy helper inclusion'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'legacy fulfillment service import'],
  ['let option = FulfillmentService::new(db.clone())', 'legacy fulfillment constructor call'],
  ['.get_shipping_option(', 'legacy shipping option owner call'],
]) {
  requireText(helperSource, value, label);
}

const canonicalConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(/g) ?? [];
if (canonicalConstructors.length !== 1) {
  failures.push(
    `expected one canonical fulfillment constructor in the facade, found ${canonicalConstructors.length}`,
  );
}
const legacyConstructors = helperSource.match(/FulfillmentService::new\(/g) ?? [];
if (legacyConstructors.length !== 1) {
  failures.push(
    `expected one legacy fulfillment constructor routed through the facade, found ${legacyConstructors.length}`,
  );
}

for (const [value, label] of [
  ['"validate_selected_shipping_option"', 'public operation mapping'],
  ['"Selected shipping option is invalid"', 'unchanged public message'],
  ['"SHIPPING_OPTION_INVALID"', 'unchanged public code'],
  ['false,', 'unchanged public retryability'],
]) {
  requireText(publicFacade, value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL cart shipping option context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Legacy GraphQL cart shipping option lookup retains typed fulfillment owner diagnostics while the public SHIPPING_OPTION_INVALID envelope remains unchanged',
);
