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
  ['struct AdminReconciliationErrorContext {', 'typed route context'],
  ['tenant_id: Uuid,', 'typed tenant identity'],
  ['actor_id: Uuid,', 'typed actor identity'],
  ['provider_operation_id: Option<Uuid>,', 'typed optional operation identity'],
  ["operation: &'static str,", 'static route operation'],
  ['struct AdminReconciliationDiagnosticContext {', 'diagnostic context'],
  ['tenant_id: uuid_shape(context.tenant_id)', 'tenant shape projection'],
  ['actor_id: uuid_shape(context.actor_id)', 'actor shape projection'],
  [
    'provider_operation_id: optional_uuid_shape(context.provider_operation_id)',
    'operation shape projection',
  ],
  ['struct AdminReconciliationDiagnosticError;', 'redacted diagnostic error'],
  ['formatter.write_str("redacted")', 'redacted debug output'],
  ["fn uuid_shape(value: Uuid) -> &'static str", 'required UUID shape helper'],
  ["fn optional_uuid_shape(value: Option<Uuid>) -> &'static str", 'optional UUID shape helper'],
  ['"nil"', 'nil shape'],
  ['"non_nil"', 'non-nil shape'],
  ['"absent"', 'absent optional shape'],
  ['"present_nil"', 'present nil optional shape'],
  ['"present_non_nil"', 'present non-nil optional shape'],
  ['[Permission::FULFILLMENTS_MANAGE]', 'manage permission'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['"list_reconciliation_required"', 'list operation context'],
  ['"quarantine_stale_executing"', 'quarantine operation context'],
  ['"resolve_unknown_as_failed"', 'resolve-failed operation context'],
  ['"resolve_unknown_as_succeeded"', 'resolve-succeeded operation context'],
  ['"retry_local_persistence"', 'retry-local operation context'],
  ['"retry_create_label"', 'retry-label operation context'],
  ['auth.user_id', 'truthful actor identity'],
  ['Some(operation_id)', 'truthful provider operation identity'],
  ['map_reconciliation_fulfillment_error(', 'owner mapper handoff'],
  ['map_reconciliation_orchestration_error(', 'orchestration mapper handoff'],
  ['map_provider_result_encoding_error(context, error)', 'encoding mapper handoff'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition variant'],
  ['FulfillmentError::Database(_)', 'database variant'],
  [
    'let context = AdminReconciliationDiagnosticContext::from(&context);',
    'owner diagnostic projection',
  ],
  ['let error = AdminReconciliationDiagnosticError;', 'owner error shadow'],
  ['error = ?error', 'redacted owner error event'],
  ['tenant_id = %context.tenant_id', 'bounded tenant log'],
  ['actor_id = %context.actor_id', 'bounded actor log'],
  [
    'provider_operation_id = %context.provider_operation_id',
    'bounded optional operation log',
  ],
  ['operation = %context.operation', 'bounded route operation log'],
  ['owner = ADMIN_RECONCILIATION_FULFILLMENT_OWNER', 'owner log'],
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
requireBefore(
  ownerMapper,
  'FulfillmentError::Validation(_)',
  'let context = AdminReconciliationDiagnosticContext::from(&context);',
  'owner typed policy before projection',
);
requireBefore(
  ownerMapper,
  'let error = AdminReconciliationDiagnosticError;',
  'tracing::error!(',
  'owner error shadow before event',
);

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
  [
    'let context = AdminReconciliationDiagnosticContext::from(&context);',
    'orchestration diagnostic projection',
  ],
  ['let error = AdminReconciliationDiagnosticError;', 'orchestration error shadow'],
  ['owner = ADMIN_RECONCILIATION_ORCHESTRATION_OWNER', 'orchestration owner log'],
  ['tenant_id = %context.tenant_id', 'bounded orchestration tenant log'],
  ['actor_id = %context.actor_id', 'bounded orchestration actor log'],
  [
    'provider_operation_id = %context.provider_operation_id',
    'bounded orchestration operation identity log',
  ],
  ['"Fulfillment operation requires reconciliation"', 'static reconciliation envelope'],
  ['HttpError::new(status, code, message)', 'single orchestration envelope constructor'],
]) requireText(orchestrationMapper, value, label);
requireBefore(
  orchestrationMapper,
  'FulfillmentOrchestrationError::OrderNotFound(_)',
  'let context = AdminReconciliationDiagnosticContext::from(&context);',
  'orchestration typed policy before projection',
);
requireBefore(
  orchestrationMapper,
  'let error = AdminReconciliationDiagnosticError;',
  'tracing::error!(',
  'orchestration error shadow before event',
);

for (const [value, label] of [
  ['_error: serde_json::Error', 'typed encoding error retained'],
  ['axum::http::StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed encoding status'],
  [
    '"commerce_admin_fulfillment_reconciliation_encoding_failed"',
    'stable encoding code',
  ],
  [
    'let context = AdminReconciliationDiagnosticContext::from(&context);',
    'encoding diagnostic projection',
  ],
  ['let error = AdminReconciliationDiagnosticError;', 'encoding error shadow'],
  ['tenant_id = %context.tenant_id', 'bounded encoding tenant log'],
  ['actor_id = %context.actor_id', 'bounded encoding actor log'],
  [
    'provider_operation_id = %context.provider_operation_id',
    'bounded encoding operation identity log',
  ],
  ['operation = %context.operation', 'encoding operation log'],
  ['error_kind = "encoding"', 'encoding kind log'],
  [
    '"Fulfillment reconciliation result could not be processed safely"',
    'static fail-closed encoding envelope',
  ],
]) requireText(encodingMapper, value, label);
requireBefore(
  encodingMapper,
  'let error = AdminReconciliationDiagnosticError;',
  'tracing::error!(',
  'encoding error shadow before event',
);

for (const value of [
  'tenant_id = %tenant_id',
  'actor_id = %actor_id',
  'provider_operation_id = ?provider_operation_id',
  'provider_operation_id = %provider_operation_id',
  'error.to_string()',
  'HttpError::bad_request(',
  'format!("failed to serialize provider result:',
]) forbidText(source, value, 'unsafe reconciliation diagnostic or public mapping');

for (const [value, expected, label] of [
  ['AdminReconciliationErrorContext::new(', 6, 'six route contexts'],
  ['map_reconciliation_fulfillment_error(', 6, 'owner mapper definition and five handoffs'],
  ['map_reconciliation_orchestration_error(', 3, 'orchestration mapper definition and two handoffs'],
  ['map_provider_result_encoding_error(', 2, 'encoding mapper definition and handoff'],
]) {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['.route("/reconciliation", get(list_reconciliation_required))', 'list route'],
  ['.route("/quarantine-stale", post(quarantine_stale_executing))', 'quarantine route'],
  ['.route("/{id}/resolve-failed", post(resolve_unknown_as_failed))', 'resolve-failed route'],
  ['"/{id}/resolve-succeeded"', 'resolve-succeeded route'],
  ['.route("/{id}/retry-local", post(retry_local_persistence))', 'retry-local route'],
  ['.route("/{id}/retry-create-label", post(retry_create_label))', 'retry-label route'],
  ['FulfillmentProviderOperationRecovery::new(runtime.db_clone())', 'recovery owner construction'],
  ['FulfillmentReconciliationService::new(runtime.db_clone())', 'local reconciliation construction'],
  ['FulfillmentCreateLabelRecoveryService::new(runtime.db_clone())', 'label recovery construction'],
  ['.with_provider_registry(runtime.fulfillment_provider_registry())', 'provider registry composition'],
  ['input.limit.unwrap_or(100)', 'bounded input limit'],
  ['input.stale_after_seconds.clamp(60, 7 * 24 * 60 * 60)', 'stale interval clamp'],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error('Commerce admin fulfillment reconciliation diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin fulfillment reconciliation retains typed policies and emits bounded diagnostics',
);
