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
  'fn map_inventory_reservation_identity_local_port_error(',
  'durable reservation wrapper implementation',
);
const reserve = between(
  wrapperImpl,
  'async fn reserve_inventory_by_identity(',
  'async fn release_inventory_by_identity(',
  'durable reserve operation',
);
const release = wrapperImpl.slice(
  wrapperImpl.indexOf('async fn release_inventory_by_identity('),
);
const mapper = between(
  wrapper,
  'fn map_inventory_reservation_identity_local_port_error(',
  'fn require_inventory_reservation_write_admission(',
  'durable reservation local mapper',
);
const legacyIdentityImpl = between(
  legacy,
  'impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {',
  'async fn load_tenant_variant<C>(',
  'legacy durable reservation implementation',
);
const legacyHelpers = legacy.slice(legacy.indexOf('async fn load_tenant_variant<C>('));

for (const [block, config] of [
  [
    reserve,
    {
      operation: 'RESERVE_OPERATION',
      delegation: '.reserve_inventory_by_identity(context, request)',
      facts: [
        'let variant_id = Some(request.variant_id);',
        'let quantity = Some(request.quantity);',
        'let line_item_id = request.line_item_id;',
        'variant_id,\n                quantity,\n                line_item_id,',
      ],
      label: 'durable reserve local routing',
    },
  ],
  [
    release,
    {
      operation: 'RELEASE_OPERATION',
      delegation: '.release_inventory_by_identity(context, request)',
      facts: [
        'None,\n                None,\n                None,',
      ],
      label: 'durable release local routing',
    },
  ],
]) {
  for (const [value, detail] of [
    [`require_inventory_reservation_write_admission(&context, ${config.operation})?;`, 'admission'],
    [`parse_inventory_reservation_tenant_id(&context, ${config.operation})?;`, 'tenant validation'],
    ['let diagnostic_context = context.clone();', 'delegated context retention'],
    ['let reservation_id = request.reservation_id;', 'reservation identity retention'],
    ['let external_id_length = request.external_id.chars().count();', 'external identity length retention'],
    ['let result = self', 'owner result retention'],
    [config.delegation, 'unchanged owner delegation'],
    ['result.map_err(|error| {', 'post-delegation mapping'],
    ['map_inventory_reservation_identity_local_port_error(', 'local mapper call'],
    ['&diagnostic_context,', 'retained context mapper argument'],
    [config.operation, 'exact operation mapper argument'],
    ['reservation_id,', 'reservation identity mapper argument'],
    ['external_id_length,', 'external identity length mapper argument'],
  ]) requireText(block, value, `${config.label} ${detail}`);
  for (const value of config.facts) requireText(block, value, `${config.label} request facts`);

  const indexes = [
    block.indexOf('require_inventory_reservation_write_admission('),
    block.indexOf('parse_inventory_reservation_tenant_id('),
    block.indexOf('let diagnostic_context = context.clone();'),
    block.indexOf(config.delegation),
    block.indexOf('map_inventory_reservation_identity_local_port_error('),
  ];
  if (!indexes.every((value, index) => index === 0 || indexes[index - 1] < value)) {
    failures.push(
      `${config.label}: expected admission -> tenant validation -> context retention -> delegation -> local mapping ordering`,
    );
  }
}

for (const [value, label] of [
  ['"inventory.reservation_external_id_invalid"', 'external identity validation code'],
  ['"reservation external_id must contain 1 to 191 characters"', 'external identity validation message'],
  [') => "normalize_external_id"', 'external identity local operation'],
  ['"inventory.reservation_quantity_invalid"', 'quantity validation code'],
  ['"reservation quantity must be positive"', 'quantity validation message'],
  [') if operation == RESERVE_OPERATION => "validate_reservation_quantity"', 'reserve quantity local operation'],
  ['("inventory.variant_not_found", "inventory variant was not found") => "load_variant"', 'variant lookup outcome for both operations'],
  ['"inventory.state_not_found"', 'inventory state code'],
  ['"variant has no configured inventory state"', 'inventory state message'],
  ['"load_inventory_state"', 'inventory state local operation'],
  ['"reservation identity is already bound to different reservation data"', 'reserve replay conflict message'],
  ['"validate_existing_reservation_identity"', 'reserve replay conflict local operation'],
  ['"inventory.insufficient_inventory"', 'reserve stock code'],
  ['"insufficient inventory for reservation"', 'reserve stock message'],
  ['"reserve_available_stock"', 'reserve stock local operation'],
  ['"inventory.reservation_not_found"', 'release not-found code'],
  ['"inventory reservation was not found"', 'release not-found message'],
  ['"load_reservation"', 'release not-found local operation'],
  ['"reservation id is bound to another external identity"', 'release identity conflict message'],
  ['"validate_release_external_identity"', 'release identity local operation'],
  ['"inventory.reservation_item_missing"', 'missing reservation item code'],
  ['"reservation inventory item is missing"', 'missing reservation item message'],
  ['"load_reservation_inventory_item"', 'missing reservation item local operation'],
  ['"reservation identity changed while acquiring the owner lock"', 'locked identity conflict message'],
  ['"revalidate_release_identity"', 'locked identity local operation'],
  ['"inventory.reservation_ledger_inconsistent"', 'ledger invariant code'],
  ['"inventory reservation ledger is inconsistent"', 'ledger invariant message'],
  ['"release_reserved_quantity"', 'ledger invariant local operation'],
  ['"inventory.available_quantity_overflow"', 'available quantity invariant code'],
  ['"inventory available quantity is outside the supported range"', 'available quantity invariant message'],
  ['"calculate_available_quantity"', 'available quantity local operation'],
  ['"inventory.database_unavailable"', 'storage code'],
  ['"inventory storage is temporarily unavailable"', 'storage message'],
  ['"owner_storage"', 'storage local operation'],
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
  ['reservation_id = %reservation_id', 'reservation identity context'],
  ['variant_id = ?variant_id', 'variant context'],
  ['request_quantity = ?quantity', 'quantity context'],
  ['line_item_id = ?line_item_id', 'line-item context'],
  ['external_id_length,', 'external identity length context'],
  ['internal_code = %error.code', 'stable local code'],
  ['internal_message = %error.message', 'stable local message'],
  ['error_kind = ?error.kind', 'typed local kind'],
  ['retryable = error.retryable', 'local retryability'],
  ['boundary = INVENTORY_RESERVATION_BOUNDARY', 'local boundary'],
  ['\n    error\n}', 'same delegated error return'],
]) requireText(mapper, value, label);

const unknownReturns = mapper.match(/_ => return error,/g)?.length ?? 0;
if (unknownReturns !== 1) {
  failures.push(`unknown local outcome pass-through count: expected 1, found ${unknownReturns}`);
}
forbidText(mapper, 'external_id =', 'raw external identity diagnostics');
forbidText(mapper, '%request.external_id', 'raw request external identity diagnostics');
forbidText(mapper, 'inventory.context_invalid', 'admission and context errors must not be remapped locally');

for (const [value, label] of [
  ['request.external_id = normalize_external_id(request.external_id)?;', 'legacy external identity normalization'],
  ['"inventory.reservation_quantity_invalid"', 'legacy quantity validation'],
  ['"inventory.state_not_found"', 'legacy inventory state conflict'],
  ['"inventory.insufficient_inventory"', 'legacy reserve stock conflict'],
  ['"reservation identity is already bound to different reservation data"', 'legacy reserve replay conflict'],
  ['"inventory.reservation_not_found"', 'legacy release not-found'],
  ['"reservation id is bound to another external identity"', 'legacy release external identity conflict'],
  ['"inventory.reservation_item_missing"', 'legacy missing inventory item invariant'],
  ['"reservation identity changed while acquiring the owner lock"', 'legacy locked identity conflict'],
  ['"inventory.reservation_ledger_inconsistent"', 'legacy ledger invariant'],
]) requireText(legacyIdentityImpl, value, label);

for (const [value, label] of [
  ['"inventory.variant_not_found",\n                "inventory variant was not found",', 'legacy variant envelope'],
  ['"inventory.available_quantity_overflow",\n                "inventory available quantity is outside the supported range",', 'legacy available quantity invariant'],
  ['"inventory.reservation_external_id_invalid"', 'legacy external identity envelope'],
  ['"inventory.database_unavailable",\n        "inventory storage is temporarily unavailable",', 'legacy storage envelope'],
]) requireText(legacyHelpers, value, label);

if (failures.length > 0) {
  console.error('Durable inventory reservation local-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Durable inventory reservation operations retain delegated context for stable local owner outcomes without logging raw external identity or changing PortError results',
);
