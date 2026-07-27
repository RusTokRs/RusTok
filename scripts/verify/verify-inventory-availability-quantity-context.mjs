#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const wrapper = read('crates/rustok-inventory/src/reservation_port_context.rs');
const legacy = read('crates/rustok-inventory/src/ports.rs');
const lib = read('crates/rustok-inventory/src/lib.rs');
const journaled = read('crates/rustok-commerce/src/services/journaled_checkout.rs');
const staged = read('crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs');
const legacyStorefront = read('crates/rustok-commerce/src/storefront_checkout_runtime.rs');
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
  'impl InventoryReservationPort for InProcessInventoryReservationPort {',
  'fn require_inventory_reservation_read_admission(',
  'inventory availability and quantity wrapper implementation',
);
const availability = between(
  wrapperImpl,
  'async fn check_availability(',
  'async fn reserve_inventory(',
  'availability operation',
);
const reserve = between(
  wrapperImpl,
  'async fn reserve_inventory(',
  'async fn release_inventory_reservation(',
  'quantity reserve operation',
);
const release = wrapperImpl.slice(
  wrapperImpl.indexOf('async fn release_inventory_reservation('),
);
const readAdmission = between(
  wrapper,
  'fn require_inventory_reservation_read_admission(',
  'fn require_inventory_reservation_write_admission(',
  'read admission helper',
);
const writeAdmission = between(
  wrapper,
  'fn require_inventory_reservation_write_admission(',
  'fn log_inventory_reservation_admission_rejection(',
  'write admission helper',
);
const admissionLog = between(
  wrapper,
  'fn log_inventory_reservation_admission_rejection(',
  'fn parse_inventory_reservation_tenant_id(',
  'admission diagnostics',
);
const tenantValidation = wrapper.slice(
  wrapper.indexOf('fn parse_inventory_reservation_tenant_id('),
);
const legacyImpl = between(
  legacy,
  'impl InventoryReservationPort for crate::InventoryService {',
  'impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {',
  'legacy availability and quantity implementation',
);
const stagedComposition = between(
  staged,
  'let event_bus = runtime.event_bus();',
  'let marketplace_allocation_service = Arc::new(',
  'mounted staged storefront inventory composition',
);

for (const [value, label] of [
  ['mod reservation_port_context;', 'private context adapter module'],
  ['pub use crate::reservation_port_context::{', 'public adapter facade'],
  ['InProcessInventoryReservationPort, in_process_inventory_reservation_port,', 'root adapter exports'],
  ['PersistentInventoryReservationIdentityPort,', 'durable wrapper export preserved'],
  ['in_process_inventory_reservation_identity_port,', 'durable factory export preserved'],
]) requireText(lib, value, label);

for (const [value, label] of [
  ['const INVENTORY_OWNER: &str = "rustok_inventory";', 'truthful owner'],
  ['const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_port";', 'stable boundary'],
  ['const AVAILABILITY_OPERATION: &str = "check_availability";', 'availability operation'],
  ['const RESERVE_OPERATION: &str = "reserve_inventory";', 'reserve operation'],
  ['const RELEASE_OPERATION: &str = "release_inventory_reservation";', 'release operation'],
  ['pub struct InProcessInventoryReservationPort {', 'public adapter struct'],
  ['inner: Arc<dyn InventoryReservationPort>', 'inner owner port'],
  ['Arc::new(crate::InventoryService::new(db, event_bus))', 'legacy service construction'],
  ['pub fn in_process_inventory_reservation_port(', 'canonical factory'],
]) requireText(wrapper, value, label);

for (const [block, admission, operation, delegation, label] of [
  [
    availability,
    'require_inventory_reservation_read_admission',
    'AVAILABILITY_OPERATION',
    'self.inner.check_availability(context, request).await',
    'availability routing',
  ],
  [
    reserve,
    'require_inventory_reservation_write_admission',
    'RESERVE_OPERATION',
    'self.inner.reserve_inventory(context, request).await',
    'quantity reserve routing',
  ],
  [
    release,
    'require_inventory_reservation_write_admission',
    'RELEASE_OPERATION',
    '.release_inventory_reservation(context, request)',
    'quantity release routing',
  ],
]) {
  const admissionCall = `${admission}(&context, ${operation})?;`;
  const tenantCall = `parse_inventory_reservation_tenant_id(&context, ${operation})?;`;
  requireText(block, admissionCall, label);
  requireText(block, tenantCall, label);
  requireText(block, delegation, label);
  const admissionIndex = block.indexOf(admissionCall);
  const tenantIndex = block.indexOf(tenantCall);
  const delegationIndex = block.indexOf(delegation);
  if (!(admissionIndex < tenantIndex && tenantIndex < delegationIndex)) {
    failures.push(`${label}: expected admission -> tenant validation -> owner delegation ordering`);
  }
}

requireText(
  readAdmission,
  'context.require_policy(PortCallPolicy::read()).map_err(|error| {',
  'read-policy interception',
);
forbidText(readAdmission, 'require_write_semantics()', 'read operation must not require write semantics');

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::write()).map_err(|error| {', 'write-policy interception'],
  ['context.require_write_semantics().map_err(|error| {', 'write-semantics interception'],
  ['"policy"', 'policy phase'],
  ['"write_semantics"', 'write-semantics phase'],
  ['error\n    })', 'same admission error return'],
]) requireText(writeAdmission, value, label);
requireText(readAdmission, 'error\n    })', 'same read admission error return');

for (const [value, label] of [
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical admission event'],
  ['tracing::warn!(', 'ordinary admission event'],
  ['error = ?error', 'original admission error evidence'],
  ['owner = INVENTORY_OWNER', 'truthful admission owner'],
  ['operation,', 'exact admission operation'],
  ['admission_phase,', 'admission phase'],
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
]) requireText(admissionLog, value, label);

for (const [value, label] of [
  ['Uuid::parse_str(context.tenant_id.trim()).map_err(|cause| {', 'trimmed tenant parsing'],
  ['let error = PortError::validation(', 'stable context envelope construction'],
  ['"inventory.context_invalid"', 'stable context code'],
  ['"inventory request context is invalid"', 'stable context message'],
  ['parse_cause = ?cause', 'tenant parse cause'],
  ['error = ?error', 'mapped tenant error'],
  ['owner = INVENTORY_OWNER', 'truthful tenant owner'],
  ['validation_phase = "tenant_id"', 'tenant validation phase'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'tenant boundary'],
  ['error\n    })', 'same tenant error return'],
]) requireText(tenantValidation, value, label);

for (const [value, label] of [
  ['let owner_operation = "check_availability";', 'legacy availability operation'],
  ['let owner_operation = "reserve_inventory";', 'legacy reserve operation'],
  ['let owner_operation = "release_inventory_reservation";', 'legacy release operation'],
  ['.check_variant_availability_for_channel(', 'legacy availability delegation'],
  ['.reserve(tenant_id, request.variant_id, request.quantity)', 'legacy quantity reserve delegation'],
  ['.release_reservation_quantity(tenant_id, request.variant_id, request.quantity)', 'legacy quantity release delegation'],
  ['inventory_error_to_port_error(&context, owner_operation, error)', 'legacy owner error mapping'],
]) requireText(legacyImpl, value, label);

requireText(
  journaled,
  'rustok_inventory::in_process_inventory_reservation_port(',
  'journaled compatibility cutover',
);
forbidText(
  journaled,
  'rustok_inventory::InventoryService::new(',
  'journaled direct inventory service composition',
);

for (const [value, label] of [
  ['let inventory_availability = rustok_inventory::in_process_inventory_reservation_port(', 'mounted staged canonical inventory factory'],
  ['runtime.db_clone(),', 'mounted staged database delegation'],
  ['event_bus.clone(),', 'mounted staged event bus delegation'],
  ['rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone())', 'mounted staged durable reservation factory preserved'],
  ['let plan_builder = crate::CheckoutPlanBuilder::new(', 'mounted staged plan builder preserved'],
]) requireText(stagedComposition, value, label);
forbidText(
  stagedComposition,
  'rustok_inventory::InventoryService::new(',
  'mounted staged direct inventory service composition',
);

requireText(
  legacyStorefront,
  'rustok_inventory::InventoryService::new(',
  'dead-code legacy storefront gap',
);

for (const [pattern, expected, label] of [
  [/impl InventoryReservationPort for InProcessInventoryReservationPort/g, 1, 'adapter trait impl count'],
  [/require_inventory_reservation_read_admission\(/g, 2, 'read admission definition/use count'],
  [/require_inventory_reservation_write_admission\(/g, 3, 'write admission definition/use count'],
  [/parse_inventory_reservation_tenant_id\(/g, 4, 'tenant validation definition/use count'],
]) {
  const count = wrapper.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

if (failures.length > 0) {
  console.error('Inventory availability and quantity context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Canonical inventory availability and quantity adapter retains full admission and tenant context across journaled and mounted staged storefront composition, with only the dead-code legacy storefront gap explicit',
);
