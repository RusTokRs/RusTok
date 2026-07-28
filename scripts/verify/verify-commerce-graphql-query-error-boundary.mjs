#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { readCommerceSafeQuerySource } from './lib/commerce-safe-query-source.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mod.rs');
const facade = readCommerceSafeQuerySource(read);
const source = read('crates/rustok-commerce/src/graphql/query.rs');
const orderErrors = read('crates/rustok-order/src/error.rs');
const paymentErrors = read('crates/rustok-payment/src/error.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(routing, '#[path = "safe_query.rs"]\nmod query;', 'safe query routing');
if ((routing.match(/\bmod query;/g) ?? []).length !== 1) {
  failures.push('expected one query module declaration');
}

for (const [value, label] of [
  ['mod query_error_boundary;', 'query boundary module'],
  ['pub(crate) enum BoundaryError {', 'local boundary error'],
  ['Graphql(Error)', 'GraphQL pass-through'],
  ['Public {', 'static public envelope'],
  ['pub(crate) trait QueryGraphqlMessage', 'constructor policy'],
  ['impl QueryGraphqlMessage for String', 'dynamic string redaction'],
  ['impl QueryGraphqlMessage for &str', 'static message preservation'],
  ['impl From<Error> for BoundaryError', 'GraphQL conversion'],
  ['impl From<String> for BoundaryError', 'string conversion'],
  ['impl From<sea_orm::DbErr> for BoundaryError', 'database conversion'],
  ['impl From<crate::CommerceError> for BoundaryError', 'commerce conversion'],
]) requireText(facade, value, label);

for (const [value, label] of [
  ['impl From<FulfillmentError> for BoundaryError', 'fulfillment conversion'],
  ['impl From<OrderError> for BoundaryError', 'order conversion'],
  ['impl From<PaymentError> for BoundaryError', 'payment conversion'],
  ['impl From<BoundaryError> for Error', 'GraphQL restoration'],
  ['extensions.set("code", code)', 'code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'dynamic safe code'],
  ['"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE"', 'database safe code'],
  ['error_message = %self', 'dynamic error logging'],
  ['boundary = "commerce_graphql_query"', 'boundary logging'],
  ['pub(crate) const MODULE_SLUG: &str = super::MODULE_SLUG;', 'module forwarding'],
  ['pub(crate) const PRODUCT_MODULE_SLUG: &str = super::PRODUCT_MODULE_SLUG;', 'product forwarding'],
  ['pub(crate) fn map_product_service_error(', 'product mapper forwarding'],
  ['pub(crate) fn product_query_tenant(', 'tenant helper forwarding'],
  ['pub(crate) fn require_commerce_permission(', 'permission forwarding'],
  ['pub(crate) async fn require_storefront_channel_enabled(', 'channel forwarding'],
  ['mod source;', 'source module'],
  ['mod async_graphql_shim {', 'GraphQL shim'],
  ['use self::async_graphql_shim as async_graphql;', 'GraphQL alias'],
  ['pub type Error = super::super::query_error_boundary::BoundaryError;', 'custom Error'],
  ['pub type FieldError = super::super::query_error_boundary::BoundaryError;', 'custom FieldError'],
  ['mod rustok_api_shim {', 'API shim'],
  ['mod rustok_fulfillment_shim;', 'fulfillment shim'],
  ['use self::rustok_api_shim as rustok_api;', 'API alias'],
  ['use self::rustok_fulfillment_shim as rustok_fulfillment;', 'fulfillment alias'],
  ['include!("../query.rs");', 'unchanged query inclusion'],
  ['pub use source::CommerceQuery;', 'query export'],
]) requireText(facade, value, label);

for (const value of [
  'BoundaryError::Graphql(error) => error',
  'BoundaryError::Graphql(Error::new(self))',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::unauthenticated()',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message)',
  '::rustok_api::graphql::require_module_enabled(ctx, module_slug)',
]) requireText(facade, value, 'existing GraphQL preservation');

for (const value of [
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'Error::new(format!("{error}"))',
]) forbidText(facade, value, 'facade dynamic public constructor');

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'fulfillment option not-found'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment conflict'],
  ['FulfillmentError::Database(_)', 'fulfillment unavailable'],
  ['"FULFILLMENT_REQUEST_INVALID"', 'fulfillment validation code'],
  ['"FULFILLMENT_RESOURCE_NOT_FOUND"', 'fulfillment not-found code'],
  ['"FULFILLMENT_STATE_CONFLICT"', 'fulfillment conflict code'],
  ['"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'fulfillment unavailable code'],
  ['"FULFILLMENT_ACCESS_DENIED"', 'fulfillment forbidden code'],
  ['"FULFILLMENT_OPERATION_FAILED"', 'fulfillment invariant code'],
  ['OrderError::Validation(_)', 'order validation'],
  ['OrderError::OrderNotFound(_)', 'order not-found'],
  ['OrderError::InvalidTransition { .. }', 'order conflict'],
  ['OrderError::Database(_)', 'order unavailable'],
  ['OrderError::Core(_)', 'order internal'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order not-found code'],
  ['"ORDER_STATE_CONFLICT"', 'order conflict code'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order unavailable code'],
  ['"ORDER_OPERATION_FAILED"', 'order internal code'],
  ['PaymentError::Validation(_)', 'payment validation'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'payment collection not-found'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'provider invalid response'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'provider unknown outcome'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration'],
  ['"PAYMENT_REQUEST_INVALID"', 'payment validation code'],
  ['"PAYMENT_RESOURCE_NOT_FOUND"', 'payment not-found code'],
  ['"PAYMENT_STATE_CONFLICT"', 'payment conflict code'],
  ['"PAYMENT_TEMPORARILY_UNAVAILABLE"', 'payment unavailable code'],
  ['"PAYMENT_RECONCILIATION_REQUIRED"', 'payment reconciliation code'],
  ['"PAYMENT_CONFIGURATION_ERROR"', 'payment configuration code'],
]) requireText(facade, value, label);

for (const [ownerSource, value, label] of [
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order not-found'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return not-found'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change not-found'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'owner collection not-found'],
  [paymentErrors, 'ProviderUnavailable {', 'owner provider unavailable'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'owner provider unknown'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner option not-found'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment not-found'],
]) requireText(ownerSource, value, label);

for (const value of [
  'async fn storefront_returns(',
  'async fn storefront_refunds(',
  'async fn storefront_order_changes(',
  'async fn storefront_payment_collection(',
  'async fn order(',
  'async fn orders(',
  'async fn payment_collection(',
  'async fn refunds(',
  'async fn shipping_option(',
  'async fn fulfillments(',
]) requireText(source, value, 'unchanged resolver source');

const dynamicPatterns = [
  /async_graphql::Error::new\(err\.to_string\(\)\)/g,
  /async_graphql::Error::new\(error\.message\)/g,
  /err\.to_string\(\)\.into\(\)/g,
  /error\.message\.into\(\)/g,
  /async_graphql::Error::new\(format!\(/g,
];
const dynamicSites = dynamicPatterns.reduce(
  (total, pattern) => total + (source.match(pattern) ?? []).length,
  0,
);
if (dynamicSites < 10) failures.push(`expected unchanged resolver compatibility sites, found ${dynamicSites}`);
if ((facade.match(/include!\("\.\.\/query\.rs"\)/g) ?? []).length !== 1) {
  failures.push('expected one unchanged query source include');
}

for (const code of [
  'COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE',
  'FULFILLMENT_TEMPORARILY_UNAVAILABLE',
  'ORDER_TEMPORARILY_UNAVAILABLE',
  'PAYMENT_TEMPORARILY_UNAVAILABLE',
]) {
  const policy = new RegExp(`"${code}"[\\s\\S]{0,80}true`);
  if (!policy.test(facade)) failures.push(`retryable temporary envelope missing for ${code}`);
}
if (!/"PAYMENT_RECONCILIATION_REQUIRED"[\s\S]{0,80}false/.test(facade)) {
  failures.push('non-retryable reconciliation envelope missing');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL query error-boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL query errors remain isolated behind stable typed envelopes while query.rs stays unchanged',
);
