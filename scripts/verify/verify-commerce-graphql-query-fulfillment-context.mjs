#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const query = read('crates/rustok-commerce/src/graphql/query.rs');
const facade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['mod rustok_fulfillment_shim {', 'private fulfillment facade'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'safe fulfillment routing'],
  ['inner: ::rustok_fulfillment::FulfillmentService', 'canonical fulfillment delegate'],
  ['::rustok_fulfillment::FulfillmentService::new(db)', 'canonical constructor isolation'],
  ['pub async fn list_shipping_options(', 'shipping option list facade'],
  ['pub async fn find_by_order(', 'order fulfillment facade'],
  ['FulfillmentResult<Vec<ShippingOptionResponse>>', 'shipping option typed result'],
  ['FulfillmentResult<Option<FulfillmentResponse>>', 'order fulfillment typed result'],
  ['"storefront_shipping_options"', 'storefront query field'],
  ['"list_shipping_options"', 'shipping option owner operation'],
  ['"order"', 'admin order query field'],
  ['"find_by_order"', 'order fulfillment owner operation'],
  ['FulfillmentError::Validation(_)', 'validation classification'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found classification'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found classification'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition classification'],
  ['FulfillmentError::Database(_)', 'database classification'],
  ['owner = "rustok_fulfillment"', 'truthful owner'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['query_field,', 'query field context'],
  ['operation,', 'owner operation context'],
  ['order_id = ?order_id', 'order context'],
  ['requested_locale = ?requested_locale', 'requested locale context'],
  ['tenant_default_locale = ?tenant_default_locale', 'default locale context'],
  ['owner_code,', 'stable owner code'],
  ['owner_kind,', 'owner kind'],
  ['owner_retryable,', 'owner retryability'],
  ['"commerce_graphql_query_fulfillment_facade"', 'explicit facade boundary'],
  ['error\n                    })', 'original typed error rethrow'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['use rustok_fulfillment::FulfillmentService;', 'unchanged query import'],
  ['let mut options = FulfillmentService::new(db.clone())', 'storefront constructor call'],
  ['.list_shipping_options(', 'storefront shipping option call'],
  ['let fulfillment = FulfillmentService::new(db.clone())', 'admin order constructor call'],
  ['.find_by_order(tenant_id, id)', 'admin order fulfillment call'],
  ['.map_err(|err| async_graphql::Error::new(err.to_string()))?', 'existing admin order public conversion'],
]) {
  requireText(query, value, label);
}

const sourceConstructors = query.match(/FulfillmentService::new\(db\.clone\(\)\)/g) ?? [];
if (sourceConstructors.length !== 2) {
  failures.push(`expected exactly two query fulfillment constructors, found ${sourceConstructors.length}`);
}

const canonicalConstructors =
  facade.match(/::rustok_fulfillment::FulfillmentService::new\(db\)/g) ?? [];
if (canonicalConstructors.length !== 1) {
  failures.push(`expected one canonical fulfillment constructor, found ${canonicalConstructors.length}`);
}

for (const value of [
  '"Fulfillment query is invalid"',
  '"FULFILLMENT_REQUEST_INVALID"',
  '"Fulfillment resource was not found"',
  '"FULFILLMENT_RESOURCE_NOT_FOUND"',
  '"Fulfillment state conflicts with this query"',
  '"FULFILLMENT_STATE_CONFLICT"',
  '"Fulfillment data is temporarily unavailable"',
  '"FULFILLMENT_TEMPORARILY_UNAVAILABLE"',
  '"Commerce query could not be completed safely"',
  '"COMMERCE_QUERY_OPERATION_FAILED"',
]) {
  requireText(facade, value, 'existing public GraphQL envelope');
}

forbidText(
  query,
  '::rustok_fulfillment::FulfillmentService',
  'query source must remain facade-routed',
);

if (failures.length > 0) {
  console.error('Commerce GraphQL query fulfillment context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL query fulfillment reads retain typed owner diagnostics before existing public mappings',
);
