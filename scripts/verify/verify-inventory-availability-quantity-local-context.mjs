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
const service = read('crates/rustok-inventory/src/services/inventory.rs');
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
  'fn map_inventory_reservation_local_port_error(',
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
const mapper = between(
  wrapper,
  'fn map_inventory_reservation_local_port_error(',
  'fn require_inventory_reservation_read_admission(',
  'inventory local outcome mapper',
);
const legacyMapper = legacy.slice(legacy.indexOf('fn inventory_error_to_port_error('));

for (const [block, config] of [
  [
    availability,
    {
      admission: 'require_inventory_reservation_read_admission(&context, AVAILABILITY_OPERATION)?;',
      tenant: 'parse_inventory_reservation_tenant_id(&context, AVAILABILITY_OPERATION)?;',
      quantity: 'let quantity = request.requested_quantity;',
      delegation: 'self.inner.check_availability(context, request).await',
      operation: 'AVAILABILITY_OPERATION',
      label: 'availability local routing',
    },
  ],
  [
    reserve,
    {
      admission: 'require_inventory_reservation_write_admission(&context, RESERVE_OPERATION)?;',
      tenant: 'parse_inventory_reservation_tenant_id(&context, RESERVE_OPERATION)?;',
      quantity: 'let quantity = request.quantity;',
      delegation: 'self.inner.reserve_inventory(context, request).await',
      operation: 'RESERVE_OPERATION',
      label: 'quantity reserve local routing',
    },
  ],
  [
    release,
    {
      admission: 'require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;',
      tenant: 'parse_inventory_reservation_tenant_id(&context, RELEASE_OPERATION)?;',
      quantity: 'let quantity = request.quantity;',
      delegation: '.release_inventory_reservation(context, request)',
      operation: 'RELEASE_OPERATION',
      label: 'quantity release local routing',
    },
  ],
]) {
  for (const [value, detail] of [
    [config.admission, 'admission'],
    [config.tenant, 'tenant validation'],
    ['let diagnostic_context = context.clone();', 'delegated context retention'],
    ['let variant_id = request.variant_id;', 'variant retention'],
    [config.quantity, 'quantity retention'],
    ['let result = self', 'owner result retention'],
    [config.delegation, 'unchanged owner delegation'],
    ['result.map_err(|error| {', 'post-delegation mapping'],
    ['map_inventory_reservation_local_port_error(', 'local mapper call'],
    ['&diagnostic_context,', 'retained context mapper argument'],
    [config.operation, 'exact operation mapper argument'],
    ['variant_id,', 'variant mapper argument'],
    ['quantity,', 'quantity mapper argument'],
  ]) requireText(block, value, `${config.label} ${detail}`);

  const indexes = [
    block.indexOf(config.admission),
    block.indexOf(config.tenant),
    block.indexOf('let diagnostic_context = context.clone();'),
    block.indexOf(config.delegation),
    block.indexOf('map_inventory_reservation_local_port_error('),
  ];
  if (!indexes.every((value, index) => index === 0 || indexes[index - 1] < value)) {
    failures.push(
      `${config.label}: expected admission -> tenant validation -> context retention -> delegation -> local mapping ordering`,
    );
  }
}

for (const [value, label] of [
  ['("inventory.validation", "inventory request is invalid")', 'stable validation envelope'],
  ['AVAILABILITY_OPERATION => "validate_availability_request"', 'availability validation outcome'],
  ['RESERVE_OPERATION => "validate_reservation_request"', 'reserve validation outcome'],
  ['RELEASE_OPERATION => "validate_reservation_release_request"', 'release validation outcome'],
  ['("inventory.variant_not_found", "inventory variant was not found") => "load_variant"', 'variant lookup outcome'],
  ['"inventory.insufficient_inventory"', 'insufficient inventory code'],
  ['"inventory reservation conflicts with available stock"', 'insufficient inventory message'],
  [') if operation == RESERVE_OPERATION => "reserve_available_stock"', 'reserve-only stock outcome'],
  ['"inventory.database_unavailable"', 'storage code'],
  ['"inventory storage is temporarily unavailable"', 'storage message'],
  [') => "owner_storage"', 'storage local operation'],
  ['"inventory.invariant_violation"', 'invariant code'],
  ['"inventory operation violated an owner invariant"', 'invariant message'],
  [') => "owner_invariant"', 'invariant local operation'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['tracing::error!(', 'technical local event'],
  ['tracing::warn!(', 'ordinary local event'],
  ['error = ?error', 'original delegated error evidence'],
  ['owner = INVENTORY_OWNER', 'truthful local owner'],
  ['operation,', 'exact public operation'],
  ['local_operation,', 'exact local operation'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['variant_id = %variant_id', 'variant context'],
  ['request_quantity = quantity', 'quantity context'],
  ['internal_code = %error.code', 'stable local code'],
  ['internal_message = %error.message', 'stable local message'],
  ['error_kind = ?error.kind', 'typed local kind'],
  ['retryable = error.retryable', 'local retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'local boundary'],
  ['\n    error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);

const unknownReturns = mapper.match(/_ => return error,/g)?.length ?? 0;
if (unknownReturns !== 2) {
  failures.push(`unknown local outcome pass-through count: expected 2, found ${unknownReturns}`);
}
forbidText(mapper, 'inventory.context_invalid', 'admission and context errors must not be remapped locally');

for (const [value, label] of [
  ['PortError::unavailable(\n                "inventory.database_unavailable",\n                "inventory storage is temporarily unavailable",', 'legacy stable storage envelope'],
  ['"inventory.variant_not_found",\n                "inventory variant was not found",', 'legacy stable variant envelope'],
  ['"inventory.insufficient_inventory",\n                "inventory reservation conflicts with available stock",', 'legacy stable stock envelope'],
  ['PortError::validation("inventory.validation", "inventory request is invalid")', 'legacy stable validation envelope'],
  ['"inventory.invariant_violation",\n                "inventory operation violated an owner invariant",', 'legacy stable invariant envelope'],
]) requireText(legacyMapper, value, label);

for (const [value, label] of [
  ['validate_availability_request_quantity(requested_quantity)?;', 'availability request validation source'],
  ['validate_reservation_quantity(quantity)?;', 'reserve request validation source'],
  ['validate_release_quantity(quantity)?;', 'release request validation source'],
  ['let variant = self.load_variant', 'variant lookup source'],
  ['CommerceError::InsufficientInventory {', 'insufficient inventory source'],
  ['return Err(insufficient_reserved_release_error(quantity, 0));', 'release validation source'],
]) requireText(service, value, label);

if (failures.length > 0) {
  console.error('Inventory availability and quantity local-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Inventory availability and deprecated quantity operations retain delegated context for stable local owner outcomes and return the same PortError',
);
