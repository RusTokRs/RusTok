#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL(
    'crates/rustok-commerce/src/services/checkout_inventory_reservation_executor.rs',
    root,
  ),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  [
    'const CHECKOUT_INVENTORY_BOUNDARY: &str = "commerce_checkout_inventory_reservation";',
    'stable checkout inventory boundary',
  ],
  ['PortErrorKind,', 'typed port error classification import'],
  ['let port_context = inventory_port_context(InventoryPortContextInput {', 'retained port context'],
  ['port_context.clone()', 'same context delegated to owner port'],
  ['&port_context,\n                            "reserve_inventory_by_identity"', 'reserve operation context'],
  ['&port_context,\n                                    "release_inventory_by_identity"', 'release operation context'],
  ['fn log_checkout_inventory_boundary_failure(', 'shared structured diagnostic helper'],
  ['owner = "rustok_inventory"', 'truthful owner identity'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['cart_line_item_id = %cart_line_item_id', 'cart line identity'],
  ['reservation_id = %reservation_id', 'reservation identity'],
  ['code = %boundary_error.code', 'stable port code'],
  ['error_kind = ?boundary_error.kind', 'typed port error kind'],
  ['retryable = boundary_error.retryable', 'port retryability'],
  ['boundary = CHECKOUT_INVENTORY_BOUNDARY', 'explicit boundary identity'],
  ['"checkout inventory owner boundary failed"', 'error diagnostic event'],
  ['"checkout inventory owner boundary was rejected"', 'warning diagnostic event'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'error severity classification'],
]) {
  requireText(value, label);
}

const retainedContexts =
  source.match(/let port_context = inventory_port_context\(InventoryPortContextInput \{/g) ?? [];
if (retainedContexts.length !== 2) {
  failures.push(`expected two retained checkout inventory contexts, found ${retainedContexts.length}`);
}

const delegatedContexts = source.match(/port_context\.clone\(\)/g) ?? [];
if (delegatedContexts.length !== 2) {
  failures.push(`expected two cloned contexts delegated to inventory owner, found ${delegatedContexts.length}`);
}

const boundaryCalls = source.match(/\.record_boundary_failure\(/g) ?? [];
if (boundaryCalls.length !== 4) {
  failures.push(`expected four inventory boundary failure paths, found ${boundaryCalls.length}`);
}

const helperIndex = source.indexOf('log_checkout_inventory_boundary_failure(');
const journalIndex = source.indexOf('.reservation_journal\n            .record_error(', helperIndex);
if (helperIndex < 0 || journalIndex < helperIndex) {
  failures.push('structured diagnostics must run before the existing reservation journal write');
}

for (const [value, label] of [
  ['boundary.code.clone()', 'journal code preservation'],
  ['boundary.message.clone()', 'journal message preservation'],
  ['code: boundary.code', 'public error code preservation'],
  ['message: boundary.message', 'public error message preservation'],
  ['retryable: boundary.retryable', 'public retryability preservation'],
  ['CheckoutInventoryExecutionError::Boundary {', 'boundary error variant'],
  ['CheckoutInventoryExecutionError::BoundaryAndJournal {', 'boundary and journal error variant'],
  ['"inventory.reservation_response_mismatch"', 'reserve mismatch code'],
  ['"inventory.release_response_mismatch"', 'release mismatch code'],
  ['.with_causation_id(input.operation_id.to_string())', 'causation construction'],
  ['.with_idempotency_key(input.idempotency_key.to_string())', 'idempotency construction'],
  ['.with_deadline(input.deadline)', 'deadline construction'],
]) {
  requireText(value, label);
}

forbidText(
  '.reserve_inventory_by_identity(\n                    inventory_port_context(',
  'inline reserve context that cannot be reused for diagnostics',
);
forbidText(
  '.release_inventory_by_identity(\n                            inventory_port_context(',
  'inline release context that cannot be reused for diagnostics',
);

if (failures.length > 0) {
  console.error('Checkout inventory boundary context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout inventory reserve/release failures retain the complete owner PortContext before journaling',
);
