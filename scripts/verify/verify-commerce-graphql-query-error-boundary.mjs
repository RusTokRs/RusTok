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
const boundary = read(
  'crates/rustok-commerce/src/graphql/safe_query/query_error_boundary.rs',
);
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
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const requireBefore = (content, first, second, label) => {
  const firstIndex = content.indexOf(first);
  const secondIndex = content.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    failures.push(`${label}: ${first} must precede ${second}`);
  }
};

const dynamicMapper = between(
  boundary,
  'impl QueryGraphqlMessage for String {',
  'impl QueryGraphqlMessage for &str {',
  'dynamic string mapper',
);
const borrowedMapper = between(
  boundary,
  'impl QueryGraphqlMessage for &str {',
  'impl QueryGraphqlMessage for BoundaryError {',
  'borrowed string mapper',
);
const databaseMapper = between(
  boundary,
  'impl From<sea_orm::DbErr> for BoundaryError {',
  'impl From<rustok_product::CommerceError> for BoundaryError {',
  'database mapper',
);
const commerceMapper = between(
  boundary,
  'impl From<crate::CommerceError> for BoundaryError {',
  'impl From<FulfillmentError> for BoundaryError {',
  'commerce mapper',
);
const fulfillmentMapper = between(
  boundary,
  'impl From<FulfillmentError> for BoundaryError {',
  'impl From<OrderError> for BoundaryError {',
  'fulfillment mapper',
);
const orderMapper = between(
  boundary,
  'impl From<OrderError> for BoundaryError {',
  'impl From<PaymentError> for BoundaryError {',
  'order mapper',
);
const paymentMapper = between(
  boundary,
  'impl From<PaymentError> for BoundaryError {',
  'impl From<BoundaryError> for Error {',
  'payment mapper',
);

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
  ['impl QueryGraphqlMessage for &str', 'borrowed string redaction'],
  ['impl From<Error> for BoundaryError', 'typed GraphQL conversion'],
  ['impl From<String> for BoundaryError', 'string conversion'],
  ['impl From<sea_orm::DbErr> for BoundaryError', 'database conversion'],
  ['impl From<crate::CommerceError> for BoundaryError', 'commerce conversion'],
  ['impl From<FulfillmentError> for BoundaryError', 'fulfillment conversion'],
  ['impl From<OrderError> for BoundaryError', 'order conversion'],
  ['impl From<PaymentError> for BoundaryError', 'payment conversion'],
  ['impl From<BoundaryError> for Error', 'GraphQL restoration'],
  ['extensions.set("code", code)', 'code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['const QUERY_ERROR_BOUNDARY: &str = "commerce_graphql_query";', 'boundary constant'],
  ['struct QueryDiagnosticError;', 'diagnostic error type'],
  ['formatter.write_str("redacted")', 'redacted diagnostic Debug'],
  ['fn text_presence_shape(value: &str)', 'text shape helper'],
]) requireText(boundary, value, label);

for (const [value, label] of [
  ['let message_presence = text_presence_shape(&self);', 'dynamic message shape'],
  ['let message_len = self.len();', 'dynamic message length'],
  ['source_owner = "commerce_graphql_query.dynamic_message"', 'dynamic source owner'],
  ['error_kind = "dynamic_message"', 'dynamic error kind'],
  ['message_presence,', 'dynamic presence log'],
  ['message_len,', 'dynamic length log'],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'dynamic safe code'],
  ['"Commerce query could not be completed safely"', 'dynamic safe message'],
]) requireText(dynamicMapper, value, label);
for (const value of ['error_message = %self', 'message = %self', 'error = %self']) {
  forbidText(dynamicMapper, value, 'raw dynamic string diagnostic');
}

for (const [value, label] of [
  ['let message_presence = text_presence_shape(self);', 'borrowed message shape'],
  ['let message_len = self.len();', 'borrowed message length'],
  ['source_owner = "commerce_graphql_query.borrowed_message"', 'borrowed source owner'],
  ['error_kind = "borrowed_message"', 'borrowed error kind'],
  ['message_presence,', 'borrowed presence log'],
  ['message_len,', 'borrowed length log'],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'borrowed safe code'],
  ['"Commerce query could not be completed safely"', 'borrowed safe message'],
  ['retryable = false', 'borrowed retryability'],
]) requireText(borrowedMapper, value, label);
for (const value of [
  'BoundaryError::Graphql(Error::new(self))',
  'Error::new(self)',
  'error_message = %self',
  'message = %self',
  'error = %self',
]) {
  forbidText(borrowedMapper, value, 'borrowed message bypass');
}

for (const mapper of [
  ['dynamic', dynamicMapper],
  ['borrowed', borrowedMapper],
  ['database', databaseMapper],
  ['commerce', commerceMapper],
  ['fulfillment', fulfillmentMapper],
  ['order', orderMapper],
  ['payment', paymentMapper],
]) {
  const [label, content] = mapper;
  requireText(content, 'let error = QueryDiagnosticError;', `${label} diagnostic shadow`);
  requireText(content, 'error = ?error', `${label} redacted error log`);
  requireText(content, 'boundary = QUERY_ERROR_BOUNDARY', `${label} boundary log`);
  requireBefore(
    content,
    'let error = QueryDiagnosticError;',
    'tracing::error!(',
    `${label} shadow order`,
  );
}

requireText(databaseMapper, 'fn from(_error: sea_orm::DbErr)', 'discarded database cause');
requireText(databaseMapper, 'owner = "sea_orm"', 'database owner');
requireText(
  databaseMapper,
  '"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE"',
  'database safe code',
);
requireText(databaseMapper, '"Commerce data is temporarily unavailable"', 'database message');

requireText(commerceMapper, 'match &error {', 'borrowed commerce policy selection');
requireText(commerceMapper, 'owner = "rustok_commerce"', 'commerce owner');
for (const value of [
  'crate::CommerceError::Database(_)',
  '"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE"',
  '"COMMERCE_QUERY_OPERATION_FAILED"',
]) requireText(commerceMapper, value, 'commerce policy');
requireBefore(
  commerceMapper,
  'let (message, code, retryable, error_kind) = match &error',
  'let error = QueryDiagnosticError;',
  'commerce policy before projection',
);

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
  ['owner = "rustok_fulfillment"', 'fulfillment owner'],
]) requireText(fulfillmentMapper, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation'],
  ['OrderError::OrderNotFound(_)', 'order not-found'],
  ['OrderError::OrderReturnNotFound(_)', 'return not-found'],
  ['OrderError::OrderChangeNotFound(_)', 'change not-found'],
  ['OrderError::InvalidTransition { .. }', 'order conflict'],
  ['OrderError::Database(_)', 'order unavailable'],
  ['OrderError::Core(_)', 'order internal'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order not-found code'],
  ['"ORDER_STATE_CONFLICT"', 'order conflict code'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order unavailable code'],
  ['"ORDER_OPERATION_FAILED"', 'order internal code'],
  ['owner = "rustok_order"', 'order owner'],
]) requireText(orderMapper, value, label);

for (const [value, label] of [
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
  ['owner = "rustok_payment"', 'payment owner'],
]) requireText(paymentMapper, value, label);

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
  'BoundaryError::Graphql(error) => error',
  'impl From<Error> for BoundaryError',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::unauthenticated()',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message)',
  '::rustok_api::graphql::require_module_enabled(ctx, module_slug)',
  'pub(crate) const MODULE_SLUG: &str = super::MODULE_SLUG;',
  'pub(crate) const PRODUCT_MODULE_SLUG: &str = super::PRODUCT_MODULE_SLUG;',
  'pub(crate) fn map_product_service_error(',
  'pub(crate) fn product_query_tenant(',
  'pub(crate) fn require_commerce_permission(',
  'pub(crate) async fn require_storefront_channel_enabled(',
  'mod source;',
  'include!("../query.rs");',
  'pub use source::CommerceQuery;',
]) requireText(facade, value, 'existing typed GraphQL preservation');

for (const value of [
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'Error::new(format!("{error}"))',
]) forbidText(facade, value, 'facade dynamic public constructor');

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
if (dynamicSites < 10) {
  failures.push(`expected unchanged resolver compatibility sites, found ${dynamicSites}`);
}
if ((facade.match(/include!\("\.\.\/query\.rs"\)/g) ?? []).length !== 1) {
  failures.push('expected one unchanged query source include');
}

const shadows = boundary.match(/let error = QueryDiagnosticError;/g) ?? [];
if (shadows.length !== 7) {
  failures.push(`expected seven diagnostic shadows, found ${shadows.length}`);
}
if ((boundary.match(/error = \?error/g) ?? []).length !== 7) {
  failures.push('expected seven redacted diagnostic error fields');
}
for (const value of [
  'error_message = %self',
  'error = ?_error',
  'error = ?self',
  'boundary = "commerce_graphql_query"',
]) forbidText(boundary, value, 'unsafe or duplicated diagnostic source');

for (const code of [
  'COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE',
  'FULFILLMENT_TEMPORARILY_UNAVAILABLE',
  'ORDER_TEMPORARILY_UNAVAILABLE',
  'PAYMENT_TEMPORARILY_UNAVAILABLE',
]) {
  const policy = new RegExp(`"${code}"[\\s\\S]{0,80}true`);
  if (!policy.test(boundary)) {
    failures.push(`retryable temporary envelope missing for ${code}`);
  }
}
if (!/"PAYMENT_RECONCILIATION_REQUIRED"[\s\S]{0,80}false/.test(boundary)) {
  failures.push('non-retryable reconciliation envelope missing');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL query diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL query errors retain typed GraphQL pass-through while borrowed and owned messages use bounded stable envelopes',
);
