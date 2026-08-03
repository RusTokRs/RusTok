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
const tenantEvidence = JSON.parse(read(
  'crates/rustok-payment/contracts/evidence/payment-collection-tenant-diagnostic-safety-source.json',
));
const ownerEvidence = JSON.parse(read(
  'crates/rustok-payment/contracts/evidence/payment-collection-owner-error-diagnostic-safety-source.json',
));
const doc = read('crates/rustok-payment/docs/payment-collection-tenant-context.md');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const countText = (content, value) => content.split(value).length - 1;
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const createBlock = between(source, '    async fn create_or_reuse_collection(', '    async fn read_collection_status(', 'create implementation');
const readBlock = between(source, '    async fn read_collection_status(', '\n}\n\nfn require_payment_collection_read_admission(', 'status implementation');
const admissionBlock = between(source, 'fn require_payment_collection_read_admission(', 'fn parse_port_tenant_id(', 'admission block');
const tenantParser = between(source, 'fn parse_port_tenant_id(', '#[derive(Debug)]\nstruct PaymentCollectionOwnerErrorFacts', 'tenant parser');
const ownerMapperStart = source.indexOf('#[derive(Debug)]\nstruct PaymentCollectionOwnerErrorFacts');
const ownerMapper = ownerMapperStart >= 0 ? source.slice(ownerMapperStart) : '';
if (ownerMapperStart < 0) failures.push('owner mapper contract: unable to isolate source block');

for (const [content, values, label] of [
  [createBlock, [
    'let owner_operation = CREATE_OR_REUSE_COLLECTION_OPERATION;',
    'require_payment_collection_write_admission(&context, owner_operation)?;',
    'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
    'find_reusable_collection_by_cart(tenant_id, cart_id)',
  ], 'create tenant ordering'],
  [readBlock, [
    'let owner_operation = READ_COLLECTION_STATUS_OPERATION;',
    'require_payment_collection_read_admission(&context, owner_operation)?;',
    'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
    'get_collection(tenant_id, request.collection_id)',
  ], 'read tenant ordering'],
]) {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value);
    if (index < 0) failures.push(`${label}: missing ${value}`);
    if (index >= 0 && index <= previous) failures.push(`${label}: ${value} is out of order`);
    previous = index;
  }
}

if ((source.match(/parse_port_tenant_id\(&context, owner_operation\)\?/g) ?? []).length !== 2) {
  failures.push('expected exactly two operation-aware tenant parser calls');
}

for (const [value, label] of [
  ['Uuid::parse_str(&context.tenant_id).map_err(|parse_error| {', 'tenant UUID parsing'],
  ['let error = PortError::validation(', 'stable validation construction'],
  ['"payment.tenant_id_invalid"', 'stable validation code'],
  ['"PortContext.tenant_id must be a UUID for payment ports"', 'stable validation message'],
  ['let parse_error_type = std::any::type_name_of_val(&parse_error);', 'type-only parse cause'],
  ['tenant_id_parse_failed = true', 'parse-failure fact'],
  ['tenant_id_length', 'tenant shape'],
  ['actor_kind', 'actor kind'],
  ['claim_count', 'claim count'],
  ['role_count', 'role count'],
  ['channel_present', 'channel presence'],
  ['locale_length', 'locale length'],
  ['causation_id_present', 'causation presence'],
  ['traceparent_present', 'trace presence'],
  ['idempotency_key_present', 'idempotency presence'],
  ['internal_message_present', 'message presence'],
  ['internal_message_length', 'message length'],
  ['let error_kind = "validation";', 'closed kind'],
  ['correlation_id = %context.correlation_id', 'correlation'],
  ['operation = owner_operation', 'owner operation'],
  ['validation = "tenant_id"', 'validation phase'],
  ['boundary = PAYMENT_COLLECTION_PORT_BOUNDARY', 'boundary'],
  ['"payment collection tenant context was rejected"', 'event'],
]) requireText(tenantParser, value, label);
for (const value of [
  'parse_error = ?parse_error', 'parse_error = %parse_error', 'error = ?error',
  'error = %error', 'tenant_id = %context.tenant_id', 'actor = ?context.actor',
  'channel = ?context.channel', 'locale = %context.locale',
  'causation_id = ?context.causation_id', 'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key', 'internal_message = %error.message',
  'error_kind = ?error.kind',
]) forbidText(tenantParser, value, 'unsafe tenant payload');
if (countText(tenantParser, 'tracing::warn!(') !== 1) failures.push('tenant parser must have one warning path');

for (const marker of [
  'context.require_policy(PortCallPolicy::read())',
  'context.require_policy(PortCallPolicy::write())',
  'context.require_write_semantics()',
  'log_payment_collection_admission_rejection(',
]) requireText(admissionBlock, marker, 'admission remains mounted');

for (const marker of [
  'struct PaymentCollectionOwnerErrorFacts',
  'payment_collection_owner_error_facts(&error)',
  'owner_error_variant = error_facts.error_variant',
  'owner_error_text_total_length = error_facts.text_total_length',
  'owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'owner_error_opaque_payload_present = error_facts.opaque_payload_present',
  '"payment collection was not found"',
  '"payment was not found"',
  '"refund was not found"',
]) requireText(ownerMapper, marker, 'separately closed owner mapper');
for (const value of [
  'cause = %message', 'provider_id = %provider_id', 'provider_operation = %operation',
  'from = %from', 'to = %to', 'error = ?error', 'tenant_id = %context.tenant_id',
  'format!("payment collection {id} not found")',
  'format!("payment for collection {id} not found")',
  'format!("refund {id} not found")',
]) forbidText(ownerMapper, value, 'owner mapper regression');

if (tenantEvidence.status !== 'payment_collection_tenant_diagnostic_safety_source_unvalidated') {
  failures.push(`tenant evidence status mismatch: ${tenantEvidence.status}`);
}
for (const [key, expected] of Object.entries({
  tenant_parser_bounded: true,
  complete_parse_error_logged: false,
  constructed_port_error_logged: false,
  tenant_internal_message_text_logged: false,
  tenant_context_shape_only: true,
  tenant_correlation_preserved: true,
  tenant_owner_operations_preserved: true,
  tenant_validation_phase_preserved: true,
  tenant_error_kind_closed: true,
  tenant_warning_severity_preserved: true,
  same_validation_port_error_returned: true,
  tenant_parser_call_sites_preserved: true,
  admission_mapper_changed: false,
  canonical_payment_error_mapper_cleanup_out_of_scope: true,
  execution_behavior_changed: false,
  public_port_error_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) if (tenantEvidence.source_contract?.[key] !== expected) failures.push(`tenant evidence ${key} must be ${expected}`);

if (ownerEvidence.status !== 'payment_collection_owner_error_diagnostic_safety_source_unvalidated') {
  failures.push(`owner evidence status mismatch: ${ownerEvidence.status}`);
}
if (ownerEvidence.source_contract?.complete_payment_error_logged !== false ||
    ownerEvidence.source_contract?.static_not_found_public_messages !== true) {
  failures.push('owner evidence must retain bounded mapper and static not-found contract');
}

for (const [label, evidence] of [['tenant', tenantEvidence], ['owner', ownerEvidence]]) {
  if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push(`${label} execution must remain empty`);
  for (const key of ['tests_run','cargo_run','format_run','verifiers_run','workflow_checks_run','ci_run','runtime_proven']) {
    if (evidence.validation?.[key] !== false) failures.push(`${label} validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Tenant diagnostics retain only the parse-error type and bounded context/message-shape facts',
  'The same constructed validation `PortError` is returned after diagnostics',
  'Canonical `payment_error_to_port_error` is now closed by a separate source-only contract',
]) requireText(doc, marker, 'documentation');

if (failures.length > 0) {
  console.error('Payment collection tenant diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log('✔ Payment collection tenant rejection remains bounded and the canonical owner mapper is separately source-closed');
