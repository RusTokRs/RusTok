#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mod.rs');
const facade = read('crates/rustok-commerce/src/graphql/safe_query.rs');
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

requireText(
  routing,
  '#[path = "safe_query.rs"]\nmod query;',
  'safe query module routing',
);
const queryModuleDeclarations = routing.match(/\bmod query;/g) ?? [];
if (queryModuleDeclarations.length !== 1) {
  failures.push(`expected one query module declaration, found ${queryModuleDeclarations.length}`);
}

for (const [value, label] of [
  ['mod query_error_boundary {', 'query boundary module'],
  ['#[derive(Clone, Debug)]', 'async GraphQL clone requirement'],
  ['pub(crate) enum BoundaryError {', 'local boundary error'],
  ['Graphql(Error)', 'existing GraphQL pass-through variant'],
  ['Public {', 'static public envelope variant'],
  ['pub(crate) trait QueryGraphqlMessage', 'dynamic constructor policy'],
  ['impl QueryGraphqlMessage for String', 'owned string redaction'],
  ['impl QueryGraphqlMessage for &str', 'static message preservation'],
  ['impl From<Error> for BoundaryError', 'GraphQL error pass-through'],
  ['impl From<String> for BoundaryError', 'string Into redaction'],
  ['impl From<sea_orm::DbErr> for BoundaryError', 'database conversion'],
  ['impl From<crate::CommerceError> for BoundaryError', 'commerce conversion'],
  ['impl From<FulfillmentError> for BoundaryError', 'fulfillment conversion'],
  ['impl From<OrderError> for BoundaryError', 'order conversion'],
  ['impl From<PaymentError> for BoundaryError', 'payment conversion'],
  ['impl From<BoundaryError> for Error', 'GraphQL restoration'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['"COMMERCE_QUERY_OPERATION_FAILED"', 'redacted dynamic code'],
  ['"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE"', 'database temporary code'],
  ['error_message = %self', 'raw dynamic string logging'],
  ['boundary = "commerce_graphql_query"', 'query boundary logging'],
  ['pub(crate) const MODULE_SLUG: &str = super::MODULE_SLUG;', 'module slug forwarding'],
  ['pub(crate) const PRODUCT_MODULE_SLUG: &str = super::PRODUCT_MODULE_SLUG;', 'product slug forwarding'],
  ['pub(crate) mod types {', 'type forwarding module'],
  ['pub(crate) fn map_product_service_error(', 'product mapper forwarding'],
  ['pub(crate) fn product_query_tenant(', 'tenant helper forwarding'],
  ['pub(crate) fn require_commerce_permission(', 'permission helper forwarding'],
  ['pub(crate) async fn require_storefront_channel_enabled(', 'channel helper forwarding'],
  ['mod source {', 'isolated source module'],
  ['mod async_graphql_shim {', 'async GraphQL shim'],
  ['use self::async_graphql_shim as async_graphql;', 'async GraphQL shim alias'],
  ['pub type Error = super::super::query_error_boundary::BoundaryError;', 'custom Error alias'],
  ['pub type FieldError = super::super::query_error_boundary::BoundaryError;', 'custom FieldError alias'],
  ['pub type Result<T> =', 'custom Result alias'],
  ['mod rustok_api_shim {', 'rustok API shim'],
  ['use self::rustok_api_shim as rustok_api;', 'rustok API shim alias'],
  ['AuthContext, Permission, PortActor, PortContext, PortErrorKind, RequestContext,', 'rustok API type forwarding'],
  ['TenantContext, locale_tags_match,', 'rustok API context forwarding'],
  ['pub trait GraphQLError {', 'safe GraphQL helper trait'],
  ['pub async fn require_module_enabled(', 'module guard forwarding'],
  ['include!("query.rs");', 'unchanged query source inclusion'],
  ['pub use source::CommerceQuery;', 'query type export'],
]) {
  requireText(facade, value, label);
}

for (const value of [
  'BoundaryError::Graphql(error) => error',
  'BoundaryError::Graphql(Error::new(self))',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::unauthenticated()',
  '<::async_graphql::FieldError as ::rustok_api::graphql::GraphQLError>::permission_denied(message)',
  '::rustok_api::graphql::require_module_enabled(ctx, module_slug)',
]) {
  requireText(facade, value, 'existing GraphQL contract preservation');
}

for (const value of [
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'Error::new(format!("{error}"))',
]) {
  forbidText(facade, value, 'facade dynamic public constructor');
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment state mapping'],
  ['FulfillmentError::Database(_)', 'fulfillment database mapping'],
  ['"FULFILLMENT_REQUEST_INVALID"', 'fulfillment validation code'],
  ['"FULFILLMENT_RESOURCE_NOT_FOUND"', 'fulfillment not-found code'],
  ['"FULFILLMENT_STATE_CONFLICT"', 'fulfillment state code'],
  ['"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'fulfillment temporary code'],
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(_)', 'order not-found mapping'],
  ['OrderError::OrderReturnNotFound(_)', 'order-return not-found mapping'],
  ['OrderError::OrderChangeNotFound(_)', 'order-change not-found mapping'],
  ['OrderError::InvalidTransition { .. }', 'order state mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order core mapping'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order not-found code'],
  ['"ORDER_STATE_CONFLICT"', 'order state code'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order temporary code'],
  ['"ORDER_OPERATION_FAILED"', 'order safe fallback code'],
  ['PaymentError::Validation(_)', 'payment validation mapping'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found mapping'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found mapping'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found mapping'],
  ['PaymentError::InvalidTransition { .. }', 'payment state mapping'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider outage mapping'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection mapping'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'invalid provider response mapping'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'unknown provider outcome mapping'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration mapping'],
  ['PaymentError::Database(_)', 'payment database mapping'],
  ['"PAYMENT_REQUEST_INVALID"', 'payment validation code'],
  ['"PAYMENT_RESOURCE_NOT_FOUND"', 'payment not-found code'],
  ['"PAYMENT_STATE_CONFLICT"', 'payment state code'],
  ['"PAYMENT_TEMPORARILY_UNAVAILABLE"', 'payment temporary code'],
  ['"PAYMENT_RECONCILIATION_REQUIRED"', 'payment reconciliation code'],
  ['"PAYMENT_CONFIGURATION_ERROR"', 'payment configuration code'],
]) {
  requireText(facade, value, label);
}

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner order transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [paymentErrors, 'PaymentCollectionNotFound(Uuid)', 'owner collection variant'],
  [paymentErrors, 'PaymentNotFound(Uuid)', 'owner payment variant'],
  [paymentErrors, 'RefundNotFound(Uuid)', 'owner refund variant'],
  [paymentErrors, 'ProviderUnavailable {', 'owner provider outage variant'],
  [paymentErrors, 'ProviderRejected {', 'owner provider rejection variant'],
  [paymentErrors, 'ProviderInvalidResponse {', 'owner invalid response variant'],
  [paymentErrors, 'ProviderOutcomeUnknown {', 'owner unknown outcome variant'],
  [paymentErrors, 'ProviderConfiguration { provider_id: String }', 'owner configuration variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['async fn storefront_returns(', 'storefront returns resolver'],
  ['async fn storefront_refunds(', 'storefront refunds resolver'],
  ['async fn storefront_order_changes(', 'storefront order changes resolver'],
  ['async fn storefront_payment_collection(', 'storefront payment collection resolver'],
  ['async fn order(', 'admin order resolver'],
  ['async fn orders(', 'admin orders resolver'],
  ['async fn payment_collection(', 'admin payment collection resolver'],
  ['async fn refunds(', 'admin refunds resolver'],
  ['async fn shipping_option(', 'admin shipping option resolver'],
  ['async fn fulfillments(', 'admin fulfillments resolver'],
  ['async fn load_storefront_customer_order(', 'storefront ownership helper'],
]) {
  requireText(source, value, label);
}

const dynamicStringPatterns = [
  /async_graphql::Error::new\(err\.to_string\(\)\)/g,
  /async_graphql::Error::new\(error\.message\)/g,
  /err\.to_string\(\)\.into\(\)/g,
  /error\.message\.into\(\)/g,
  /async_graphql::Error::new\(format!\(/g,
];
const dynamicStringSites = dynamicStringPatterns.reduce(
  (total, pattern) => total + (source.match(pattern) ?? []).length,
  0,
);
if (dynamicStringSites < 10) {
  failures.push(`expected legacy dynamic query errors to remain isolated behind facade, found ${dynamicStringSites}`);
}

const sourceIncludes = facade.match(/include!\("query\.rs"\)/g) ?? [];
if (sourceIncludes.length !== 1) {
  failures.push(`expected one unchanged query include, found ${sourceIncludes.length}`);
}

const temporaryTrueMappings = [
  '"COMMERCE_QUERY_TEMPORARILY_UNAVAILABLE",\n                true,',
  '"FULFILLMENT_TEMPORARILY_UNAVAILABLE",\n                    true,',
  '"ORDER_TEMPORARILY_UNAVAILABLE",\n                    true,',
  '"PAYMENT_TEMPORARILY_UNAVAILABLE",\n                    true,',
];
for (const value of temporaryTrueMappings) {
  requireText(facade, value, 'retryable temporary envelope');
}
requireText(
  facade,
  '"PAYMENT_RECONCILIATION_REQUIRED",\n                    false,',
  'non-retryable reconciliation envelope',
);

if (failures.length > 0) {
  console.error('Commerce GraphQL query error-boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL query errors are isolated behind stable envelopes while the resolver source remains unchanged',
);
