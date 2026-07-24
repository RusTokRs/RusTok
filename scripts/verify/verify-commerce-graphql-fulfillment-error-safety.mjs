#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  'async_graphql::Error::new(err.to_string())',
  'async_graphql::Error::new(error.to_string())',
  'FieldError::new(err.to_string())',
  'FieldError::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
]) {
  forbidText(value, 'fulfillment mutation public boundary');
}

for (const [value, label] of [
  ['ErrorExtensions', 'GraphQL extension support'],
  ['fn public_fulfillment_graphql_error(', 'public envelope constructor'],
  ['fn order_error_envelope(', 'order envelope mapper'],
  ['fn payment_error_envelope(', 'payment envelope mapper'],
  ['fn payment_orchestration_error_envelope(', 'payment orchestration mapper'],
  ['fn order_mutation_graphql_error(', 'order mutation logger'],
  ['fn post_order_graphql_error(', 'post-order logger'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['resource_id = %resource_id', 'resource logging'],
  ['operation,', 'operation logging'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(_)', 'order not-found mapping'],
  ['OrderError::OrderReturnNotFound(_)', 'order-return mapping'],
  ['OrderError::OrderChangeNotFound(_)', 'order-change mapping'],
  ['OrderError::InvalidTransition { .. }', 'order conflict mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order fallback mapping'],
  ['PaymentError::Validation(_)', 'payment validation mapping'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'payment collection mapping'],
  ['PaymentError::PaymentNotFound(_)', 'payment mapping'],
  ['PaymentError::RefundNotFound(_)', 'refund mapping'],
  ['PaymentError::InvalidTransition { .. }', 'payment conflict mapping'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider unavailable mapping'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejected mapping'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'invalid provider response mapping'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'unknown provider outcome mapping'],
  ['PaymentError::ProviderConfiguration { .. }', 'provider configuration mapping'],
  ['PaymentError::Database(_)', 'payment database mapping'],
  ['PaymentOrchestrationError::Provider(source)', 'payment provider orchestration mapping'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation { .. }', 'reserved refund reconciliation mapping'],
  ['PaymentOrchestrationError::Payment(source)', 'payment owner orchestration mapping'],
  ['PostOrderOrchestrationError::Order(source)', 'post-order order mapping'],
  ['PostOrderOrchestrationError::Payment(source)', 'post-order payment mapping'],
  ['PostOrderOrchestrationError::PaymentOrchestration(source)', 'post-order orchestration mapping'],
  ['PostOrderOrchestrationError::Validation(_)', 'post-order validation mapping'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order resource code'],
  ['"ORDER_STATE_CONFLICT"', 'order conflict code'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order temporary code'],
  ['"ORDER_OPERATION_FAILED"', 'order fallback code'],
  ['"PAYMENT_REQUEST_INVALID"', 'payment validation code'],
  ['"PAYMENT_RESOURCE_NOT_FOUND"', 'payment resource code'],
  ['"PAYMENT_STATE_CONFLICT"', 'payment conflict code'],
  ['"PAYMENT_TEMPORARILY_UNAVAILABLE"', 'payment temporary code'],
  ['"PAYMENT_RECONCILIATION_REQUIRED"', 'payment reconciliation code'],
  ['"PAYMENT_CONFIGURATION_ERROR"', 'payment configuration code'],
  ['"POST_ORDER_REQUEST_INVALID"', 'post-order validation code'],
]) {
  requireText(value, label);
}

for (const operation of [
  'create_storefront_order_return',
  'apply_order_change',
  'create_order_return_decision',
  'complete_order_return',
]) {
  requireText(`"${operation}"`, `${operation} operation mapping`);
}

for (const mutation of [
  'async fn create_storefront_order_return(',
  'async fn mark_order_paid(',
  'async fn ship_order(',
  'async fn deliver_order(',
  'async fn cancel_order(',
  'async fn create_order_change(',
  'async fn apply_order_change(',
  'async fn cancel_order_change(',
  'async fn create_order_return(',
  'async fn create_order_return_decision(',
  'async fn complete_order_return(',
  'async fn cancel_order_return(',
]) {
  requireText(mutation, `${mutation} mutation preservation`);
}

const orderMapperCalls = source.match(/order_mutation_graphql_error\(/g) ?? [];
if (orderMapperCalls.length !== 2) {
  failures.push(`expected order mapper definition plus one call, found ${orderMapperCalls.length}`);
}
const postOrderMapperCalls = source.match(/post_order_graphql_error\(/g) ?? [];
if (postOrderMapperCalls.length !== 4) {
  failures.push(`expected post-order mapper definition plus three calls, found ${postOrderMapperCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL fulfillment error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL fulfillment mutations expose stable order/payment/post-order envelopes with internal structured logs',
);
