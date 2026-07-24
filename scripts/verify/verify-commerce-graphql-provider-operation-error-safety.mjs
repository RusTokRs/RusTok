#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-commerce/src/graphql/mutations/provider_operations.rs');
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'FieldError::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
]) {
  forbidText(value, 'provider operation public boundary');
}

for (const [value, label] of [
  ['ErrorExtensions', 'GraphQL extension support'],
  ['fn public_provider_graphql_error(', 'public envelope constructor'],
  ['fn payment_error_envelope(', 'payment owner mapper'],
  ['fn payment_orchestration_error_envelope(', 'payment orchestration mapper'],
  ['fn fulfillment_error_envelope(', 'fulfillment owner mapper'],
  ['fn fulfillment_orchestration_error_envelope(', 'fulfillment orchestration mapper'],
  ['fn payment_provider_graphql_error(', 'payment provider logger'],
  ['fn fulfillment_provider_graphql_error(', 'fulfillment provider logger'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['resource_id = %resource_id', 'resource logging'],
  ['operation,', 'operation logging'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['PaymentError::Validation(_)', 'payment validation mapping'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'payment collection not-found mapping'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found mapping'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found mapping'],
  ['PaymentError::InvalidTransition { .. }', 'payment transition mapping'],
  ['PaymentError::ProviderUnavailable { .. }', 'payment provider outage mapping'],
  ['PaymentError::ProviderRejected { .. }', 'payment provider rejection mapping'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'payment invalid response mapping'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'payment unknown outcome mapping'],
  ['PaymentError::ProviderConfiguration { .. }', 'payment configuration mapping'],
  ['PaymentError::Database(_)', 'payment database mapping'],
  ['PaymentOrchestrationError::Provider(source)', 'payment orchestration provider mapping'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation { .. }', 'payment reserved-refund mapping'],
  ['PaymentOrchestrationError::Payment(source)', 'payment orchestration owner mapping'],
  ['FulfillmentError::Validation(_)', 'fulfillment validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment transition mapping'],
  ['FulfillmentError::Database(_)', 'fulfillment database mapping'],
  ['FulfillmentOrchestrationError::OrderNotFound(_)', 'fulfillment order mapping'],
  ['FulfillmentOrchestrationError::Database(_)', 'fulfillment orchestration database mapping'],
  ['FulfillmentOrchestrationError::Fulfillment(source)', 'fulfillment owner orchestration mapping'],
  ['FulfillmentOrchestrationError::Validation(_)', 'fulfillment orchestration validation mapping'],
  ['FulfillmentOrchestrationError::ProviderAfterPersistence { .. }', 'provider-after-persistence mapping'],
  ['FulfillmentOrchestrationError::PersistenceAfterProvider { .. }', 'persistence-after-provider mapping'],
  ['"PAYMENT_REQUEST_INVALID"', 'payment validation code'],
  ['"PAYMENT_RESOURCE_NOT_FOUND"', 'payment resource code'],
  ['"PAYMENT_STATE_CONFLICT"', 'payment conflict code'],
  ['"PAYMENT_TEMPORARILY_UNAVAILABLE"', 'payment temporary code'],
  ['"PAYMENT_RECONCILIATION_REQUIRED"', 'payment reconciliation code'],
  ['"PAYMENT_CONFIGURATION_ERROR"', 'payment configuration code'],
  ['"FULFILLMENT_REQUEST_INVALID"', 'fulfillment validation code'],
  ['"FULFILLMENT_RESOURCE_NOT_FOUND"', 'fulfillment resource code'],
  ['"FULFILLMENT_STATE_CONFLICT"', 'fulfillment conflict code'],
  ['"FULFILLMENT_TEMPORARILY_UNAVAILABLE"', 'fulfillment temporary code'],
  ['"FULFILLMENT_RECONCILIATION_REQUIRED"', 'fulfillment reconciliation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'fulfillment order code'],
]) {
  requireText(value, label);
}

for (const operation of [
  'authorize_payment_collection',
  'capture_payment_collection',
  'cancel_payment_collection',
  'create_refund',
  'complete_refund',
  'cancel_refund',
  'create_fulfillment',
  'ship_fulfillment',
  'deliver_fulfillment',
  'reopen_fulfillment',
  'reship_fulfillment',
  'cancel_fulfillment',
]) {
  requireText(`"${operation}"`, `${operation} operation mapping`);
  requireText(`async fn ${operation}(`, `${operation} mutation preservation`);
}

const paymentMapperCalls = source.match(/payment_provider_graphql_error\(/g) ?? [];
if (paymentMapperCalls.length !== 7) {
  failures.push(`expected payment mapper definition plus six calls, found ${paymentMapperCalls.length}`);
}
const fulfillmentMapperCalls = source.match(/fulfillment_provider_graphql_error\(/g) ?? [];
if (fulfillmentMapperCalls.length !== 7) {
  failures.push(`expected fulfillment mapper definition plus six calls, found ${fulfillmentMapperCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL provider operation error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL payment and fulfillment provider operations expose stable public envelopes with internal structured logs',
);
