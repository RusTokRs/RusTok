#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const wrapper = read('crates/rustok-inventory/src/reservation_owner_context.rs');
const legacy = read('crates/rustok-inventory/src/ports.rs');
const lib = read('crates/rustok-inventory/src/lib.rs');
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

const wrapperImpl = between(
  wrapper,
  'impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {',
  'fn require_inventory_reservation_write_admission(',
  'durable reservation wrapper implementation',
);
const reserve = between(
  wrapperImpl,
  'async fn reserve_inventory_by_identity(',
  'async fn release_inventory_by_identity(',
  'reserve wrapper operation',
);
const release = wrapperImpl.slice(
  wrapperImpl.indexOf('async fn release_inventory_by_identity('),
);
const admission = between(
  wrapper,
  'fn require_inventory_reservation_write_admission(',
  'fn parse_inventory_reservation_tenant_id(',
  'reservation admission helpers',
);
const tenant = wrapper.slice(
  wrapper.indexOf('fn parse_inventory_reservation_tenant_id('),
);
const legacyIdentityImpl = between(
  legacy,
  'impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {',
  'async fn load_tenant_variant<C>(',
  'legacy identity reservation implementation',
);

for (const [value, label] of [
  ['#[path = "ports.rs"]', 'private legacy implementation path'],
  ['mod ports_impl;', 'private legacy implementation module'],
  ['mod reservation_owner_context;', 'private context wrapper module'],
  ['pub mod ports {', 'public compatibility facade'],
  ['pub use crate::ports_impl::{', 'public contract facade'],
  ['pub use crate::reservation_owner_context::{', 'public wrapper facade'],
  ['pub use ports::*;', 'crate-root compatibility export'],
  ['InventoryReservationIdentityPort, InventoryReservationPort,', 'trait compatibility export'],
  ['PersistentInventoryReservationIdentityPort,', 'wrapper struct compatibility export'],
  ['in_process_inventory_reservation_identity_port,', 'factory compatibility export'],
]) requireText(lib, value, label);

for (const value of [
  'pub mod ports;',
  'pub use ports_impl::*;',
  'pub use reservation_owner_context::*;',
]) forbidText(lib, value, 'public legacy bypass');

for (const [value, label] of [
  ['const INVENTORY_OWNER: &str = "rustok_inventory";', 'truthful owner'],
  ['const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_identity_port";', 'stable boundary'],
  ['const RESERVE_OPERATION: &str = "reserve_inventory_by_identity";', 'reserve operation'],
  ['const RELEASE_OPERATION: &str = "release_inventory_by_identity";', 'release operation'],
  ['pub struct PersistentInventoryReservationIdentityPort {', 'public wrapper struct'],
  ['inner: Arc<dyn InventoryReservationIdentityPort>', 'inner owner port'],
  ['crate::ports_impl::PersistentInventoryReservationIdentityPort::new(db)', 'legacy constructor delegation'],
  ['pub fn in_process_inventory_reservation_identity_port(', 'public wrapper factory'],
]) requireText(wrapper, value, label);

for (const [block, operation, delegation, label] of [
  [
    reserve,
    'RESERVE_OPERATION',
    '.reserve_inventory_by_identity(context, request)',
    'reserve owner routing',
  ],
  [
    release,
    'RELEASE_OPERATION',
    '.release_inventory_by_identity(context, request)',
    'release owner routing',
  ],
]) {
  for (const value of [
    `require_inventory_reservation_write_admission(&context, ${operation})?;`,
    `parse_inventory_reservation_tenant_id(&context, ${operation})?;`,
    delegation,
  ]) requireText(block, value, label);

  const admissionIndex = block.indexOf('require_inventory_reservation_write_admission(');
  const tenantIndex = block.indexOf('parse_inventory_reservation_tenant_id(');
  const delegationIndex = block.indexOf(delegation);
  if (!(admissionIndex >= 0 && admissionIndex < tenantIndex && tenantIndex < delegationIndex)) {
    failures.push(`${label}: expected admission -> tenant validation -> owner delegation ordering`);
  }
}

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::write()).map_err(|error| {', 'write-policy interception'],
  ['context.require_write_semantics().map_err(|error| {', 'write-semantics interception'],
  ['"policy"', 'policy phase'],
  ['"write_semantics"', 'write-semantics phase'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical admission event'],
  ['tracing::warn!(', 'ordinary admission event'],
  ['error = ?error', 'original admission error evidence'],
  ['owner = INVENTORY_OWNER', 'truthful admission owner'],
  ['operation,', 'exact admission operation'],
  ['admission_phase,', 'admission phase evidence'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['internal_code = %error.code', 'stable admission code'],
  ['internal_message = %error.message', 'stable admission message'],
  ['error_kind = ?error.kind', 'typed admission kind'],
  ['retryable = error.retryable', 'admission retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'admission boundary'],
  ['error\n    })', 'same admission error return'],
]) requireText(admission, value, label);

for (const [value, label] of [
  ['Uuid::parse_str(context.tenant_id.trim()).map_err(|cause| {', 'preserved trimmed tenant parsing'],
  ['let error = PortError::validation(', 'stable context error construction'],
  ['"inventory.context_invalid"', 'stable context code'],
  ['"inventory request context is invalid"', 'stable context message'],
  ['parse_cause = ?cause', 'tenant parse cause'],
  ['error = ?error', 'mapped context error'],
  ['owner = INVENTORY_OWNER', 'truthful context owner'],
  ['operation,', 'exact context operation'],
  ['validation_phase = "tenant_id"', 'tenant validation phase'],
  ['correlation_id = %context.correlation_id', 'tenant correlation context'],
  ['tenant_id = %context.tenant_id', 'raw delegated tenant context'],
  ['actor = ?context.actor', 'tenant actor context'],
  ['channel = ?context.channel', 'tenant channel context'],
  ['locale = %context.locale', 'tenant locale context'],
  ['causation_id = ?context.causation_id', 'tenant causation context'],
  ['traceparent = ?context.traceparent', 'tenant trace context'],
  ['idempotency_key = ?context.idempotency_key', 'tenant idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'tenant deadline context'],
  ['internal_code = %error.code', 'tenant mapped code'],
  ['internal_message = %error.message', 'tenant mapped message'],
  ['error_kind = ?error.kind', 'tenant mapped kind'],
  ['retryable = error.retryable', 'tenant mapped retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'tenant boundary'],
  ['error\n    })', 'same tenant validation error return'],
]) requireText(tenant, value, label);

for (const [value, label] of [
  ['let owner_operation = "reserve_inventory_by_identity";', 'legacy reserve operation'],
  ['let owner_operation = "release_inventory_by_identity";', 'legacy release operation'],
  ['context.require_policy(PortCallPolicy::write())?;', 'legacy write policy'],
  ['context.require_write_semantics()?;', 'legacy write semantics'],
  ['let tenant_id = parse_port_tenant_id(&context, owner_operation)?;', 'legacy tenant validation'],
  ['request.external_id = normalize_external_id(request.external_id)?;', 'legacy external identity validation'],
  ['"inventory.reservation_quantity_invalid"', 'legacy quantity validation'],
  ['"inventory.reservation_identity_conflict"', 'legacy identity conflict'],
  ['"inventory.reservation_ledger_inconsistent"', 'legacy ledger invariant'],
]) requireText(legacyIdentityImpl, value, label);

for (const [pattern, expected, label] of [
  [/impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort/g, 1, 'wrapper trait impl count'],
  [/require_inventory_reservation_write_admission\(/g, 3, 'admission definition/use count'],
  [/parse_inventory_reservation_tenant_id\(/g, 3, 'tenant validation definition/use count'],
  [/crate::ports_impl::PersistentInventoryReservationIdentityPort::new\(db\)/g, 1, 'legacy constructor delegation count'],
]) {
  const count = wrapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Inventory reservation owner context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Durable inventory reservation factories retain complete write-admission and tenant-validation context before preserving the existing owner implementation',
);
