#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/controllers/reconciliation.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
const orchestrationErrors = read(
  'crates/rustok-commerce/src/services/fulfillment_orchestration.rs',
);
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

const ownerMapper = between(
  source,
  'fn map_reconciliation_fulfillment_error(',
  'fn map_reconciliation_orchestration_error(',
  'reconciliation fulfillment mapper',
);
const orchestrationMapper = between(
  source,
  'fn map_reconciliation_orchestration_error(',
  'fn map_provider_result_encoding_error(',
  'reconciliation orchestration mapper',
);
const encodingMapper = between(
  source,
  'fn map_provider_result_encoding_error(',
  'fn require_manage_permission(',
  'provider result encoding mapper',
);

for (const [value, label] of [
  ['FulfillmentError, FulfillmentProviderOperationRecovery', 'typed fulfillment error import'],
  ['FulfillmentOrchestrationError,', 'typed orchestration error import'],
  [
    'const ADMIN_RECONCILIATION_FULFILLMENT_OWNER: &str =',
    'fulfillment owner constant',
  ],
  [
    'const ADMIN_RECONCILIATION_ORCHESTRATION_OWNER: &str =',
    'orchestration owner constant',
  ],
  [
    'const ADMIN_RECONCILIATION_BOUNDARY: &str = "commerce_admin_reconciliation_http";',
    'HTTP boundary constant',
  ],
  ['[Permission::FULFILLMENTS_MANAGE]', 'manage permission'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['"list_reconciliation_required"', 'list operation context'],
  ['"quarantine_stale_executing"', 'quarantine operation context'],
  ['"resolve_unknown_as_failed"', 'resolve-failed operation context'],
  ['"resolve_unknown_as_succeeded"', 'resolve-succeeded operation context'],
  ['"retry_local_persistence"', 'retry-local operation context'],
  ['"retry_create_label"', 'retry-label operation context'],
  ['Some(operation_id)', 'truthful provider operation identity'],
  ['map_reconciliation_fulfillment_error(', 'owner mapper handoff'],
  ['map_reconciliation_orchestration_error(', 'orchestration mapper handoff'],
  ['map_provider_result_encoding_error(tenant.id, operation_id, error)', 'encoding mapper handoff'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition variant'],
  ['FulfillmentError::Database(_)', 'database variant'],
  ['error = ?error', 'typed internal cause'],
  ['owner = ADMIN_RECONCILIATION_FULFILLMENT_OWNER', 'owner log'],
  ['tenant_id = %tenant_id', 'tenant log'],
  ['provider_operation_id = ?provider_operation_id', 'optional operation identity log'],
  ['operation,', 'owner operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_RECONCILIATION_BOUNDARY', 'boundary log'],
  ['"Fulfillment request is invalid"', 'static validation envelope'],
  ['"Commerce resource not found"', 'static not-found envelope'],
  [
    '"Fulfillment operation conflicts with the current state"',
    'static conflict envelope',
  ],
  ['"Fulfillment storage is temporarily unavailable"', 'static storage envelope'],
  ['HttpError::new(status, code, message)', 'single owner envelope constructor'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['FulfillmentOrchestrationError::Fulfillment(error)', 'nested owner delegation'],
  ['FulfillmentOrchestrationError::OrderNotFound(_)', 'order-not-found variant'],
  ['FulfillmentOrchestrationError::Database(_)', 'database variant'],
  ['FulfillmentOrchestrationError::Validation(_)', 'validation variant'],
  [
    'FulfillmentOrchestrationError::ProviderAfterPersistence { .. }',
    'provider-after-persistence variant',
  ],
  [
    'FulfillmentOrchestrationError::PersistenceAfterProvider { .. }',
    'persistence-after-provider variant',
  ],
  ['owner = ADMIN_RECONCILIATION_ORCHESTRATION_OWNER', 'orchestration owner log'],
  ['provider_operation_id = ?provider_operation_id', 'orchestration operation identity log'],
  ['"Fulfillment operation requires reconciliation"', 'static reconciliation envelope'],
  ['HttpError::new(status, code, message)', 'single orchestration envelope constructor'],
]) requireText(orchestrationMapper, value, label);

for (const [value, label] of [
  ['axum::http::StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed encoding status'],
  [
    '"commerce_admin_fulfillment_reconciliation_encoding_failed"',
    'stable encoding code',
  ],
  ['error = ?error', 'encoding cause log'],
  ['tenant_id = %tenant_id', 'encoding tenant log'],
  ['provider_operation_id = %provider_operation_id', 'encoding operation identity log'],
  ['operation = "resolve_unknown_as_succeeded"', 'encoding operation log'],
  ['error_kind = "encoding"', 'encoding kind log'],
  [
    '"Fulfillment reconciliation result could not be processed safely"',
    'static fail-closed encoding envelope',
  ],
]) requireText(encodingMapper, value, label);

for (const [ownerSource, value, label] of [
  [fulfillmentErrors, 'Validation(String)', 'owner validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner database variant'],
  [orchestrationErrors, 'OrderNotFound(Uuid)', 'orchestration order-not-found variant'],
  [orchestrationErrors, 'Database(#[from] sea_orm::DbErr)', 'orchestration database variant'],
  [orchestrationErrors, 'Fulfillment(#[from] rustok_fulfillment::error::FulfillmentError)', 'orchestration fulfillment variant'],
  [orchestrationErrors, 'Validation(String)', 'orchestration validation variant'],
  [orchestrationErrors, 'ProviderAfterPersistence {', 'provider-after-persistence variant'],
  [orchestrationErrors, 'PersistenceAfterProvider {', 'persistence-after-provider variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  'super::admin::map_fulfillment_error',
  'map_fulfillment_orchestration_error',
  'format!("failed to serialize provider result:',
  'HttpError::bad_request(',
  'error.to_string()',
]) forbidText(source, value, 'unsafe reconciliation public mapping');

if (failures.length > 0) {
  console.error('Commerce admin fulfillment reconciliation error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin fulfillment reconciliation retains typed causes and returns static public envelopes',
);
