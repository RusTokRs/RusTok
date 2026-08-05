#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  wrapper: 'crates/rustok-inventory/src/reservation_port_context.rs',
  lib: 'crates/rustok-inventory/src/lib.rs',
  journaled: 'crates/rustok-commerce/src/services/journaled_checkout.rs',
  staged: 'crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs',
  legacyStorefront: 'crates/rustok-commerce/src/storefront_checkout_runtime.rs',
  evidence:
    'crates/rustok-inventory/contracts/evidence/availability-quantity-diagnostic-safety-source-review.json',
  doc: 'crates/rustok-inventory/docs/availability-quantity-owner-context.md',
};

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) {
      failures.push(`${label}: missing ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: ${marker} is out of order`);
      return;
    }
    previous = index;
  }
}

const wrapper = read(paths.wrapper);
const lib = read(paths.lib);
const journaled = read(paths.journaled);
const staged = read(paths.staged);
const legacyStorefront = read(paths.legacyStorefront);
const evidence = JSON.parse(read(paths.evidence));
const doc = read(paths.doc);

for (const marker of [
  'mod reservation_port_context;',
  'pub use crate::reservation_port_context::{',
  'InProcessInventoryReservationPort, in_process_inventory_reservation_port,',
  'PersistentInventoryReservationIdentityPort,',
  'in_process_inventory_reservation_identity_port,',
]) requireText(lib, marker, `${paths.lib}: canonical exports`);

for (const marker of [
  'const INVENTORY_OWNER: &str = "rustok_inventory";',
  'const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_port";',
  'const AVAILABILITY_OPERATION: &str = "check_availability";',
  'const RESERVE_OPERATION: &str = "reserve_inventory";',
  'const RELEASE_OPERATION: &str = "release_inventory_reservation";',
  'pub struct InProcessInventoryReservationPort {',
  'inner: Arc<dyn InventoryReservationPort>',
  'Arc::new(crate::InventoryService::new(db, event_bus))',
  'pub fn in_process_inventory_reservation_port(',
]) requireText(wrapper, marker, `${paths.wrapper}: canonical adapter`);

const readAdmission = functionBody(wrapper, 'require_inventory_reservation_read_admission');
for (const marker of [
  'context',
  '.require_policy(PortCallPolicy::read())',
  '.inspect_err(|error|',
  'log_inventory_reservation_admission_rejection(context, operation, "policy", error);',
]) requireText(readAdmission, marker, `${paths.wrapper}: read admission`);
forbidText(readAdmission, 'require_write_semantics()', `${paths.wrapper}: read write semantics`);

const writeAdmission = functionBody(wrapper, 'require_inventory_reservation_write_admission');
for (const marker of [
  '.require_policy(PortCallPolicy::write())',
  'log_inventory_reservation_admission_rejection(context, operation, "policy", error);',
  'context.require_write_semantics().inspect_err(|error|',
  'log_inventory_reservation_admission_rejection(context, operation, "write_semantics", error);',
]) requireText(writeAdmission, marker, `${paths.wrapper}: write admission`);
requireOrder(
  writeAdmission,
  [
    '.require_policy(PortCallPolicy::write())',
    'log_inventory_reservation_admission_rejection(context, operation, "policy", error);',
    'context.require_write_semantics().inspect_err(|error|',
    'log_inventory_reservation_admission_rejection(context, operation, "write_semantics", error);',
  ],
  `${paths.wrapper}: write admission order`,
);

const admissionLogger = functionBody(wrapper, 'log_inventory_reservation_admission_rejection');
for (const marker of [
  'let technical_failure = inventory_reservation_error_is_technical(error);',
  'let context_facts = inventory_reservation_context_facts(context);',
  'if technical_failure',
  'tracing::error!(',
  'tracing::warn!(',
  'owner = INVENTORY_OWNER',
  'operation,',
  'admission_phase,',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'channel_length = ?context_facts.channel_length',
  'locale_length = context_facts.locale_length',
  'causation_id_present = context_facts.causation_id_present',
  'traceparent_present = context_facts.traceparent_present',
  'idempotency_key_present = context_facts.idempotency_key_present',
  'deadline_ms = ?context_facts.deadline_ms',
  'code = error.code.as_str()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'technical_failure,',
  'boundary = INVENTORY_RESERVATION_BOUNDARY',
]) requireText(admissionLogger, marker, `${paths.wrapper}: bounded admission logger`);
requireCount(admissionLogger, 'tracing::error!(', 1, `${paths.wrapper}: technical admission event`);
requireCount(admissionLogger, 'tracing::warn!(', 1, `${paths.wrapper}: ordinary admission event`);

const tenantParser = functionBody(wrapper, 'parse_inventory_reservation_tenant_id');
for (const marker of [
  'Uuid::parse_str(context.tenant_id.trim()).map_err(|_|',
  'let error = PortError::validation(',
  '"inventory.context_invalid"',
  '"inventory request context is invalid"',
  'log_inventory_reservation_tenant_parse_rejection(context, operation, &error);',
  '\n        error\n',
]) requireText(tenantParser, marker, `${paths.wrapper}: stable tenant parser`);
requireOrder(
  tenantParser,
  [
    'Uuid::parse_str(context.tenant_id.trim()).map_err(|_|',
    'let error = PortError::validation(',
    'log_inventory_reservation_tenant_parse_rejection(context, operation, &error);',
    '\n        error\n',
  ],
  `${paths.wrapper}: tenant parser order`,
);

const tenantLogger = functionBody(wrapper, 'log_inventory_reservation_tenant_parse_rejection');
for (const marker of [
  'tracing::warn!(',
  'owner = INVENTORY_OWNER',
  'operation,',
  'validation_phase = "tenant_id"',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id_trimmed_length = context.tenant_id.trim().chars().count()',
  'tenant_id_parse_failed = true',
  'actor_kind = context_facts.actor_kind',
  'channel_present = context_facts.channel_present',
  'code = error.code.as_str()',
  'error_message_length = error.message.chars().count()',
  'retryable = error.retryable',
  'boundary = INVENTORY_RESERVATION_BOUNDARY',
]) requireText(tenantLogger, marker, `${paths.wrapper}: bounded tenant logger`);

const diagnosticScope = `${admissionLogger}\n${tenantLogger}`;
for (const forbidden of [
  'error = ?error',
  'error = %error',
  'parse_cause =',
  'internal_message = %error.message',
  'error_kind = ?error.kind',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(diagnosticScope, forbidden, `${paths.wrapper}: raw admission/context payload`);

for (const [source, marker, label] of [
  [
    journaled,
    'rustok_inventory::in_process_inventory_reservation_port(',
    'journaled checkout canonical inventory factory',
  ],
  [
    staged,
    'rustok_inventory::in_process_inventory_reservation_port(',
    'staged storefront canonical inventory factory',
  ],
  [
    legacyStorefront,
    'rustok_inventory::in_process_inventory_reservation_port(',
    'legacy storefront canonical inventory factory',
  ],
]) {
  requireText(source, marker, label);
}
for (const [source, label] of [
  [journaled, 'journaled direct inventory composition'],
  [staged, 'staged direct inventory composition'],
  [legacyStorefront, 'legacy storefront direct inventory composition'],
]) forbidText(source, 'rustok_inventory::InventoryService::new(', label);

for (const forbidden of [
  'error = ?error',
  'parse_cause =',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'variant_id = %variant_id',
  'request_quantity = quantity',
]) forbidText(wrapper, forbidden, `${paths.wrapper}: raw diagnostic residue`);

if (
  evidence.status !==
  'inventory_availability_quantity_diagnostic_safety_source_reviewed_unvalidated'
) failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
for (const [key, expected] of Object.entries({
  admission_diagnostics_sanitized: true,
  tenant_parse_diagnostics_sanitized: true,
  uuid_parse_cause_logged: false,
  complete_port_error_logged: false,
  raw_context_logged: false,
  policy_order_preserved: true,
  write_semantics_order_preserved: true,
  tenant_trim_and_parse_preserved: true,
  tenant_public_envelope_preserved: true,
  same_admission_error_return_preserved: true,
  canonical_factory_composition_preserved: true,
  actor_validation_added: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.review_findings?.[key] !== expected) {
    failures.push(`${paths.evidence}: review_findings.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, 'Status: **source-ready / unvalidated**', `${paths.doc}: status`);
requireText(doc, 'They no longer record the full `PortError`', `${paths.doc}: admission payload policy`);
requireText(doc, 'It no longer records the UUID parser cause', `${paths.doc}: tenant payload policy`);
requireText(
  doc,
  'The source-level raw diagnostics identified in `reservation_port_context.rs` are closed.',
  `${paths.doc}: local source closure`,
);

if (failures.length > 0) {
  console.error('Inventory availability and quantity admission diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Inventory availability and quantity admission and tenant parsing preserve ordering and public errors with bounded diagnostics',
);
