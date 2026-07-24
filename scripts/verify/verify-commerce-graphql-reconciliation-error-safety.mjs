#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-commerce/src/graphql/mutations/reconciliation.rs');
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
  forbidText(value, 'refund reconciliation public boundary');
}

for (const [value, label] of [
  ['ErrorExtensions', 'GraphQL extension support'],
  ['fn public_reconciliation_graphql_error(', 'public envelope constructor'],
  ['fn payment_error_envelope(', 'payment error mapper'],
  ['fn reconciliation_graphql_error(', 'reconciliation logger'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['refund_id = %refund_id', 'refund logging'],
  ['operation,', 'operation logging'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['PaymentError::Validation(_)', 'validation mapping'],
  ['PaymentError::PaymentCollectionNotFound(_)', 'collection not-found mapping'],
  ['PaymentError::PaymentNotFound(_)', 'payment not-found mapping'],
  ['PaymentError::RefundNotFound(_)', 'refund not-found mapping'],
  ['PaymentError::InvalidTransition { .. }', 'transition mapping'],
  ['PaymentError::ProviderUnavailable { .. }', 'provider outage mapping'],
  ['PaymentError::ProviderRejected { .. }', 'provider rejection mapping'],
  ['PaymentError::ProviderInvalidResponse { .. }', 'invalid response mapping'],
  ['PaymentError::ProviderOutcomeUnknown { .. }', 'unknown outcome mapping'],
  ['PaymentError::ProviderConfiguration { .. }', 'configuration mapping'],
  ['PaymentError::Database(_)', 'database mapping'],
  ['PaymentOrchestrationError::Provider(source)', 'provider orchestration mapping'],
  ['PaymentOrchestrationError::Payment(source)', 'payment orchestration mapping'],
  ['PaymentOrchestrationError::ProviderAfterRefundReservation { .. }', 'post-reservation mapping'],
  ['"PAYMENT_RECONCILIATION_REQUEST_INVALID"', 'validation code'],
  ['"PAYMENT_RESOURCE_NOT_FOUND"', 'resource code'],
  ['"PAYMENT_RECONCILIATION_STATE_CONFLICT"', 'state conflict code'],
  ['"PAYMENT_RECONCILIATION_TEMPORARILY_UNAVAILABLE"', 'temporary code'],
  ['"PAYMENT_RECONCILIATION_REQUIRED"', 'reconciliation code'],
  ['"PAYMENT_CONFIGURATION_ERROR"', 'configuration code'],
  ['"retry_refund_provider"', 'operation label'],
  ['async fn retry_refund_provider(', 'mutation preservation'],
]) {
  requireText(value, label);
}

const mapperCalls = source.match(/reconciliation_graphql_error\(/g) ?? [];
if (mapperCalls.length !== 2) {
  failures.push(`expected reconciliation mapper definition plus one call, found ${mapperCalls.length}`);
}

const reconciliationCodeOccurrences = source.match(/"PAYMENT_RECONCILIATION_REQUIRED"/g) ?? [];
if (reconciliationCodeOccurrences.length !== 2) {
  failures.push(
    `expected unknown-outcome and post-reservation reconciliation mappings, found ${reconciliationCodeOccurrences.length}`,
  );
}

if (failures.length > 0) {
  console.error('Commerce GraphQL refund reconciliation error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL refund reconciliation exposes stable public envelopes with non-retryable unknown-outcome handling',
);
