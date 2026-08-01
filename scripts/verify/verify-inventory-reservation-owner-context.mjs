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
  wrapper: 'crates/rustok-inventory/src/reservation_owner_context.rs',
  legacy: 'crates/rustok-inventory/src/ports.rs',
  lib: 'crates/rustok-inventory/src/lib.rs',
  evidence:
    'crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json',
  review:
    'crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json',
  document: 'crates/rustok-inventory/docs/reservation-owner-context.md',
};
const wrapper = read(paths.wrapper);
const legacy = read(paths.legacy);
const lib = read(paths.lib);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
function functionBody(content, name) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${name}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`missing function ${name}`);
    return '';
  }
  const open = content.indexOf('{', match.index);
  let depth = 0;
  for (let index = open; index < content.length; index += 1) {
    if (content[index] === '{') depth += 1;
    if (content[index] === '}') {
      depth -= 1;
      if (depth === 0) return content.slice(open, index + 1);
    }
  }
  failures.push(`unterminated function ${name}`);
  return '';
}

for (const [value, label] of [
  ['#[path = "ports.rs"]', 'private legacy implementation path'],
  ['mod ports_impl;', 'private legacy implementation module'],
  ['mod reservation_owner_context;', 'private wrapper module'],
  ['pub mod ports {', 'public compatibility facade'],
  ['pub use crate::ports_impl::{', 'public owner contract facade'],
  ['pub use crate::reservation_owner_context::{', 'public wrapper facade'],
  ['pub use ports::*;', 'crate-root compatibility export'],
]) requireText(lib, value, label);

for (const value of ['pub mod ports;', 'pub use ports_impl::*;', 'pub use reservation_owner_context::*;']) {
  forbidText(lib, value, 'public legacy bypass');
}

for (const [value, label] of [
  ['const INVENTORY_OWNER: &str = "rustok_inventory";', 'truthful owner'],
  [
    'const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_identity_port";',
    'stable boundary',
  ],
  ['const RESERVE_OPERATION: &str = "reserve_inventory_by_identity";', 'reserve operation'],
  ['const RELEASE_OPERATION: &str = "release_inventory_by_identity";', 'release operation'],
  ['pub struct PersistentInventoryReservationIdentityPort {', 'wrapper struct'],
  ['inner: Arc<dyn InventoryReservationIdentityPort>', 'delegated owner port'],
  [
    'crate::ports_impl::PersistentInventoryReservationIdentityPort::new(db)',
    'legacy owner construction',
  ],
  ['pub fn in_process_inventory_reservation_identity_port(', 'canonical factory'],
  ['async fn reserve_inventory_by_identity(', 'reserve operation implementation'],
  ['async fn release_inventory_by_identity(', 'release operation implementation'],
  [
    'require_inventory_reservation_write_admission(&context, RESERVE_OPERATION)?;',
    'reserve admission',
  ],
  [
    'parse_inventory_reservation_tenant_id(&context, RESERVE_OPERATION)?;',
    'reserve tenant validation',
  ],
  [
    'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
    'release admission',
  ],
  [
    'parse_inventory_reservation_tenant_id(&context, RELEASE_OPERATION)?;',
    'release tenant validation',
  ],
  ['.reserve_inventory_by_identity(context, request)', 'reserve delegation'],
  ['.release_inventory_by_identity(context, request)', 'release delegation'],
]) requireText(wrapper, value, label);

const contextFacts = functionBody(wrapper, 'inventory_reservation_context_facts');
const kindLabel = functionBody(wrapper, 'inventory_reservation_port_error_kind');
const admission = functionBody(wrapper, 'require_inventory_reservation_write_admission');
const admissionLogger = functionBody(wrapper, 'log_inventory_reservation_admission_rejection');
const tenantParser = functionBody(wrapper, 'parse_inventory_reservation_tenant_id');
const tenantLogger = functionBody(wrapper, 'log_inventory_reservation_tenant_rejection');
const diagnosticScope = [contextFacts, kindLabel, admissionLogger, tenantLogger].join('\n');

for (const [value, label] of [
  ['tenant_id_length: context.tenant_id.chars().count()', 'tenant shape'],
  ['actor_kind', 'closed actor kind'],
  ['actor_id_length: context.actor.id.chars().count()', 'actor-id shape'],
  ['claim_count: context.claims.len()', 'claim count'],
  ['role_count: context.roles.len()', 'role count'],
  ['channel_present: context.channel.is_some()', 'channel presence'],
  ['channel_length: context.channel.as_ref().map(', 'channel length'],
  ['locale_length: context.locale.chars().count()', 'locale length'],
  ['causation_id_present: context.causation_id.is_some()', 'causation presence'],
  ['traceparent_present: context.traceparent.is_some()', 'trace presence'],
  ['idempotency_key_present: context.idempotency_key.is_some()', 'idempotency presence'],
  ['deadline_ms: context.deadline_ms', 'deadline shape'],
]) requireText(contextFacts, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation => "validation"', 'validation label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout label'],
  ['PortErrorKind::InvariantViolation => "invariant_violation"', 'invariant label'],
]) requireText(kindLabel, value, label);

for (const [value, label] of [
  ['.require_policy(PortCallPolicy::write())', 'write policy preserved'],
  ['.inspect_err(|error| {', 'write policy interception'],
  ['"policy"', 'policy phase'],
  ['context.require_write_semantics().inspect_err(|error| {', 'write semantics interception'],
  ['"write_semantics"', 'write semantics phase'],
  ['log_inventory_reservation_admission_rejection(', 'bounded admission logger call'],
]) requireText(admission, value, label);

for (const [value, label] of [
  ['tracing::error!(', 'technical event'],
  ['tracing::warn!(', 'ordinary event'],
  ['owner = INVENTORY_OWNER', 'truthful owner'],
  ['operation,', 'exact operation'],
  ['admission_phase,', 'admission phase'],
  ['correlation_id = %context.correlation_id', 'correlation id'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant shape'],
  ['actor_kind = facts.actor_kind', 'actor kind'],
  ['actor_id_length = facts.actor_id_length', 'actor shape'],
  ['claim_count = facts.claim_count', 'claim count'],
  ['role_count = facts.role_count', 'role count'],
  ['channel_present = facts.channel_present', 'channel presence'],
  ['locale_length = facts.locale_length', 'locale length'],
  ['code = %error.code', 'stable code'],
  ['error_message_present = !error.message.is_empty()', 'message presence'],
  ['error_message_length = error.message.chars().count()', 'message length'],
  [
    'error_kind = inventory_reservation_port_error_kind(&error.kind)',
    'closed error kind',
  ],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'boundary'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'severity classification',
  ],
]) requireText(admissionLogger, value, label);

for (const [value, label] of [
  ['Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {', 'trimmed UUID parsing'],
  ['let error = PortError::validation(', 'stable validation construction'],
  ['"inventory.context_invalid"', 'stable code'],
  ['"inventory request context is invalid"', 'stable message'],
  ['log_inventory_reservation_tenant_rejection(context, operation, &error);', 'bounded tenant logger'],
  ['error\n    })', 'same validation error return'],
]) requireText(tenantParser, value, label);

for (const [value, label] of [
  ['tracing::warn!(', 'warning event'],
  ['validation_phase = "tenant_id"', 'validation phase'],
  ['tenant_id_parse_failed = true', 'parse failure fact'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant shape'],
  ['code = %error.code', 'stable code'],
  ['error_message_present = !error.message.is_empty()', 'message presence'],
  ['error_message_length = error.message.chars().count()', 'message length'],
  [
    'error_kind = inventory_reservation_port_error_kind(&error.kind)',
    'closed error kind',
  ],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'boundary'],
]) requireText(tenantLogger, value, label);

for (const [value, label] of [
  ['error = ?error', 'complete PortError debug payload'],
  ['error = %error', 'complete PortError display payload'],
  ['internal_message = %error.message', 'raw PortError message'],
  ['error_kind = ?error.kind', 'debug-formatted kind'],
  ['tenant_id = %context.tenant_id', 'raw tenant'],
  ['actor = ?context.actor', 'raw actor'],
  ['channel = ?context.channel', 'raw channel'],
  ['locale = %context.locale', 'raw locale'],
  ['causation_id = ?context.causation_id', 'raw causation'],
  ['traceparent = ?context.traceparent', 'raw trace'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency'],
  ['parse_cause =', 'UUID parse cause'],
]) forbidText(diagnosticScope, value, label);

for (const marker of [
  'let owner_operation = "reserve_inventory_by_identity";',
  'let owner_operation = "release_inventory_by_identity";',
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'let tenant_id = parse_port_tenant_id(&context, owner_operation)?;',
]) requireText(legacy, marker, 'unchanged persistent owner');

for (const [key, expected] of Object.entries({
  admission_diagnostic_cleanup_closed: true,
  tenant_parser_diagnostic_cleanup_closed: true,
  complete_port_error_logged: false,
  raw_context_logged: false,
  uuid_parse_cause_logged: false,
  closed_error_kind_logged: true,
  error_message_shape_logged: true,
  admission_error_return_changed: false,
  tenant_error_return_changed: false,
  public_contract_changed: false,
  owner_delegation_changed: false,
  sql_or_state_machine_changed: false,
  inventory_ffa_fba_status_promoted: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const [key, expected] of Object.entries({
  public_facade_preserved: true,
  both_operations_preserved: true,
  admission_order_preserved: true,
  persistent_owner_rechecks_preserved: true,
  complete_admission_error_removed: true,
  raw_admission_context_removed: true,
  tenant_parse_cause_removed: true,
  bounded_admission_context_retained: true,
  bounded_tenant_context_retained: true,
  same_admission_error_returned: true,
  same_tenant_error_returned: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Inventory reservation owner diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'complete `PortError`',
  'bounded context shape',
  'UUID parse cause is not recorded',
  'The exact admission and tenant-validation errors are returned unchanged.',
  'No FBA or FFA status is promoted',
]) requireText(document, marker, 'truthful owner diagnostic document');

if (failures.length > 0) {
  console.error('Inventory reservation owner diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Durable inventory reservation admission and tenant-validation diagnostics are bounded while public routing and exact PortError returns remain unchanged',
);
