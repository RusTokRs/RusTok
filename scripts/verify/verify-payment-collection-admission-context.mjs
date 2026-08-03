#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const source = read('crates/rustok-payment/src/ports.rs');
const admissionEvidence = JSON.parse(read('crates/rustok-payment/contracts/evidence/payment-collection-admission-diagnostic-safety-source.json'));
const tenantEvidence = JSON.parse(read('crates/rustok-payment/contracts/evidence/payment-collection-tenant-diagnostic-safety-source.json'));
const ownerEvidence = JSON.parse(read('crates/rustok-payment/contracts/evidence/payment-collection-owner-error-diagnostic-safety-source.json'));
const doc = read('crates/rustok-payment/docs/payment-collection-admission-context.md');
const failures = [];

const requireText = (content, value, label) => { if (!content.includes(value)) failures.push(`${label}: missing ${value}`); };
const forbidText = (content, value, label) => { if (content.includes(value)) failures.push(`${label}: forbidden ${value}`); };
const countText = (content, value) => content.split(value).length - 1;
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) { failures.push(`${label}: unable to isolate source block`); return ''; }
  return content.slice(startIndex, endIndex);
};

const createBlock = between(source, '    async fn create_or_reuse_collection(', '    async fn read_collection_status(', 'create implementation');
const readBlock = between(source, '    async fn read_collection_status(', '\n}\n\nfn require_payment_collection_read_admission(', 'status implementation');
const readAdmission = between(source, 'fn require_payment_collection_read_admission(', 'fn require_payment_collection_write_admission(', 'read admission');
const writeAdmission = between(source, 'fn require_payment_collection_write_admission(', 'fn log_payment_collection_admission_rejection(', 'write admission');
const admissionLogger = between(source, 'fn log_payment_collection_admission_rejection(', 'fn parse_port_tenant_id(', 'admission logger');
const tenantParser = between(source, 'fn parse_port_tenant_id(', '#[derive(Debug)]\nstruct PaymentCollectionOwnerErrorFacts', 'tenant parser');
const ownerStart = source.indexOf('#[derive(Debug)]\nstruct PaymentCollectionOwnerErrorFacts');
const ownerMapper = ownerStart >= 0 ? source.slice(ownerStart) : '';
if (ownerStart < 0) failures.push('owner mapper contract: unable to isolate source block');

for (const [content, values, label] of [
  [createBlock, ['let owner_operation = CREATE_OR_REUSE_COLLECTION_OPERATION;', 'require_payment_collection_write_admission(&context, owner_operation)?;', 'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;'], 'write ordering'],
  [readBlock, ['let owner_operation = READ_COLLECTION_STATUS_OPERATION;', 'require_payment_collection_read_admission(&context, owner_operation)?;', 'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;'], 'read ordering'],
]) {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value);
    if (index < 0) failures.push(`${label}: missing ${value}`);
    if (index >= 0 && index <= previous) failures.push(`${label}: ${value} is out of order`);
    previous = index;
  }
}

for (const marker of ['context.require_policy(PortCallPolicy::read())', '.inspect_err(|error| {', 'log_payment_collection_admission_rejection(context, owner_operation, "policy", error);']) requireText(readAdmission, marker, 'read pass-through');
for (const marker of ['context.require_policy(PortCallPolicy::write())', 'log_payment_collection_admission_rejection(context, owner_operation, "policy", error);', 'context.require_write_semantics().inspect_err(|error| {', '"write_semantics",']) requireText(writeAdmission, marker, 'write pass-through');
forbidText(readAdmission, 'require_write_semantics', 'read admission');
forbidText(readAdmission + writeAdmission, 'PortError::', 'admission error reconstruction');
forbidText(readAdmission + writeAdmission, '.map_err(', 'admission error replacement');

for (const marker of [
  'error: &PortError', 'let error_kind = match &error.kind', 'PortErrorKind::Validation => "validation"',
  'PortErrorKind::NotFound => "not_found"', 'PortErrorKind::Conflict => "conflict"',
  'PortErrorKind::Forbidden => "forbidden"', 'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"', 'PortErrorKind::InvariantViolation => "invariant_violation"',
  'let technical_failure = matches!(', 'tenant_id_length', 'actor_kind', 'actor_id_length',
  'claim_count', 'role_count', 'channel_present', 'locale_length', 'causation_id_present',
  'traceparent_present', 'idempotency_key_present', 'internal_message_present',
  'internal_message_length', 'owner = PAYMENT_COLLECTION_OWNER',
  'correlation_id = %context.correlation_id', 'operation = owner_operation', 'admission,',
  'code = %error.code', 'retryable = error.retryable', 'boundary = PAYMENT_COLLECTION_PORT_BOUNDARY',
  '"payment collection admission failed"', '"payment collection admission was rejected"',
]) requireText(admissionLogger, marker, 'bounded admission logger');
for (const value of [
  'error = ?error', 'error = %error', 'tenant_id = %context.tenant_id', 'actor = ?context.actor',
  'channel = ?context.channel', 'locale = %context.locale', 'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent', 'idempotency_key = ?context.idempotency_key',
  'internal_message = %error.message', 'error_kind = ?error.kind',
]) forbidText(admissionLogger, value, 'unsafe admission payload');
if (countText(admissionLogger, 'tracing::error!(') !== 1 || countText(admissionLogger, 'tracing::warn!(') !== 1) failures.push('admission severity split must retain one error and one warning path');
if ((source.match(/log_payment_collection_admission_rejection\(/g) ?? []).length !== 4) failures.push('admission helper definition/use count must remain four');

for (const marker of ['Uuid::parse_str(&context.tenant_id).map_err(|parse_error| {', 'tenant_id_parse_failed = true', 'let parse_error_type = std::any::type_name_of_val(&parse_error);', 'validation = "tenant_id"']) requireText(tenantParser, marker, 'separate tenant contract');
for (const value of ['parse_error = ?parse_error', 'error = ?error', 'tenant_id = %context.tenant_id']) forbidText(tenantParser, value, 'tenant regression');

for (const marker of [
  'struct PaymentCollectionOwnerErrorFacts', 'payment_collection_owner_error_facts(&error)',
  'owner_error_variant = error_facts.error_variant', 'owner_error_text_total_length = error_facts.text_total_length',
  'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
  '"payment collection was not found"', '"payment was not found"', '"refund was not found"',
]) requireText(ownerMapper, marker, 'separate owner contract');
for (const value of ['cause = %message', 'provider_id = %provider_id', 'error = ?error', 'tenant_id = %context.tenant_id']) forbidText(ownerMapper, value, 'owner mapper regression');

if (admissionEvidence.status !== 'payment_collection_admission_diagnostic_safety_source_unvalidated') failures.push(`admission evidence status mismatch: ${admissionEvidence.status}`);
for (const [key, expected] of Object.entries({
  admission_mapper_bounded: true, complete_admission_port_error_logged: false,
  admission_internal_message_text_logged: false, admission_context_shape_only: true,
  admission_correlation_preserved: true, admission_owner_operations_preserved: true,
  admission_phases_preserved: true, admission_error_kind_closed: true,
  admission_severity_split_preserved: true, original_admission_port_error_returned: true,
  read_write_admission_order_preserved: true, tenant_parser_cleanup_out_of_scope: true,
  canonical_payment_error_mapper_cleanup_out_of_scope: true, execution_behavior_changed: false,
  public_port_error_changed: false, ffa_promoted: false, fba_promoted: false,
})) if (admissionEvidence.source_contract?.[key] !== expected) failures.push(`admission evidence ${key} must be ${expected}`);

if (tenantEvidence.status !== 'payment_collection_tenant_diagnostic_safety_source_unvalidated') failures.push(`tenant evidence status mismatch: ${tenantEvidence.status}`);
if (tenantEvidence.source_contract?.tenant_parser_bounded !== true) failures.push('tenant evidence must remain bounded');
if (ownerEvidence.status !== 'payment_collection_owner_error_diagnostic_safety_source_unvalidated') failures.push(`owner evidence status mismatch: ${ownerEvidence.status}`);
if (ownerEvidence.source_contract?.complete_payment_error_logged !== false || ownerEvidence.source_contract?.static_not_found_public_messages !== true) failures.push('owner evidence must retain safe mapper contract');

for (const [label, evidence] of [['admission', admissionEvidence], ['tenant', tenantEvidence], ['owner', ownerEvidence]]) {
  if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push(`${label} execution must remain empty`);
  for (const key of ['tests_run','cargo_run','format_run','verifiers_run','workflow_checks_run','ci_run','runtime_proven']) if (evidence.validation?.[key] !== false) failures.push(`${label} validation.${key} must remain false`);
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Admission diagnostics retain only bounded context and message-shape facts',
  'The exact original admission `PortError` continues through `inspect_err` unchanged',
  'Tenant UUID parsing and canonical `payment_error_to_port_error` are now closed by separate source-only contracts',
]) requireText(doc, marker, 'documentation');

if (failures.length > 0) {
  console.error('Payment collection admission diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log('✔ Payment collection admission remains bounded while tenant and owner mapper contracts are separately source-closed');
