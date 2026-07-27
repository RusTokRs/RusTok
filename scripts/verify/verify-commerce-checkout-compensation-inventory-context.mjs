#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs', root),
  'utf8',
);
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

const orchestrationBlock = between(
  source,
  '    async fn compensate_claimed(',
  '    async fn compensate_payment(',
  'compensation orchestration block',
);
const inventoryBlock = between(
  source,
  '    async fn release_remaining_reservations(',
  '    async fn release_cart(',
  'inventory release block',
);
const cartBlock = between(
  source,
  '    async fn release_cart(',
  '\n}\n\nfn cart_status(',
  'cart release block',
);
const inventoryContext = between(
  source,
  'fn inventory_context(',
  'fn cart_context(',
  'inventory context helper',
);
const ownerMapper = between(
  source,
  'fn owner_boundary_error(',
  'fn boundary_error(',
  'owner boundary mapper',
);
const boundaryMapper = between(
  source,
  'fn boundary_error(',
  'fn compensation_error_code(',
  'generic boundary mapper',
);

for (const [value, label] of [
  ['PortErrorKind', 'typed port error classification import'],
  [
    'const CHECKOUT_COMPENSATION_OWNER_BOUNDARY: &str = "checkout_compensation_owner_port";',
    'stable compensation owner boundary',
  ],
  ['const INVENTORY_COMPENSATION_OWNER: &str = "rustok_inventory";', 'truthful inventory owner'],
  [
    'const INVENTORY_COMPENSATION_OPERATION: &str = "release_inventory_by_identity";',
    'exact inventory owner operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let inventory_context = inventory_context(', 'retained inventory context'],
  ['inventory_context.clone()', 'inventory context delegation clone'],
  ['&inventory_context', 'inventory context mapper input'],
  ['INVENTORY_COMPENSATION_OWNER', 'inventory mapper owner'],
  ['INVENTORY_COMPENSATION_OPERATION', 'inventory mapper operation'],
  ['"release_inventory"', 'inventory commerce stage'],
  ['InventoryIdentityReservationReleaseRequest {', 'inventory release request'],
  ['reservation_id: reservation.reservation_id', 'reservation identity input'],
  ['external_id: reservation.external_id.clone()', 'external identity input'],
  ['released.reservation_id != reservation.reservation_id', 'reservation response identity check'],
  ['released.external_id != reservation.external_id', 'external response identity check'],
  ['released.variant_id != reservation.variant_id', 'variant response identity check'],
  ['.mark_released(tenant_id, reservation.reservation_id)', 'reservation release checkpoint'],
  ['CheckoutInventoryReservationStatus::Consumed', 'consumed reservation reconciliation'],
  ['inventory reservation {} is already consumed', 'consumed reservation message'],
]) requireText(inventoryBlock, value, label);

const contextBindings = inventoryBlock.match(/let inventory_context\s*=/g) ?? [];
const contextClones = inventoryBlock.match(/inventory_context\.clone\(\)/g) ?? [];
const mapperInputs = inventoryBlock.match(/&inventory_context/g) ?? [];
if (contextBindings.length !== 1 || contextClones.length !== 1 || mapperInputs.length !== 1) {
  failures.push(
    `expected one retained inventory context, one clone, and one mapper input, found ${contextBindings.length}/${contextClones.length}/${mapperInputs.length}`,
  );
}

for (const [value, label] of [
  ['PortActor::service("rustok-commerce.checkout-compensation")', 'inventory service actor'],
  ['PLATFORM_FALLBACK_LOCALE', 'inventory effective locale'],
  ['checkout:{}:compensation:inventory:{}', 'inventory correlation identity'],
  ['operation.id, reservation.cart_line_item_id', 'inventory correlation components'],
  ['.with_causation_id(operation.id.to_string())', 'inventory causation'],
  ['.with_idempotency_key(reservation.external_id.clone())', 'inventory idempotency key'],
  ['.with_deadline(deadline)', 'inventory deadline'],
]) requireText(inventoryContext, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ["owner: &'static str", 'owner input'],
  ["operation: &'static str", 'owner operation input'],
  ["stage: &'static str", 'commerce stage input'],
  ['error: PortError', 'original port error input'],
  [
    'log_owner_boundary_error(context, owner, operation, stage, &error);',
    'diagnostics before mapping',
  ],
  ['fn log_owner_boundary_error(', 'structured diagnostic helper'],
  ['error = ?error', 'original port error'],
  ['owner = owner', 'truthful dynamic owner'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = operation', 'exact owner operation field'],
  ['stage = stage', 'commerce stage field'],
  ['code = %error.code', 'original port code'],
  ['internal_message = %error.message', 'original public-safe port message'],
  ['error_kind = ?error.kind', 'typed port error kind'],
  ['retryable = error.retryable', 'original retryability'],
  ['boundary = CHECKOUT_COMPENSATION_OWNER_BOUNDARY', 'boundary field'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
]) requireText(ownerMapper, value, label);

const diagnosticIndex = ownerMapper.indexOf(
  'log_owner_boundary_error(context, owner, operation, stage, &error);',
);
const manualRoutingIndex = ownerMapper.indexOf('if matches!(');
const boundaryDelegationIndex = ownerMapper.indexOf('boundary_error(stage, error)');
if (
  !(
    diagnosticIndex >= 0 &&
    diagnosticIndex < manualRoutingIndex &&
    manualRoutingIndex < boundaryDelegationIndex
  )
) {
  failures.push('inventory owner diagnostics must precede unchanged owner routing and boundary mapping');
}

for (const [value, label] of [
  ['ORDER_MANUAL_RECONCILIATION_CODE | PAYMENT_MANUAL_RECONCILIATION_CODE', 'manual routing remains payment/order only'],
  ['boundary_error(stage, error)', 'inventory generic boundary delegation'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['CheckoutCompensationError::Boundary {', 'stable boundary envelope'],
  ['stage,', 'stable boundary stage'],
  ['code: error.code', 'stable boundary code'],
  ['message: error.message', 'stable boundary message'],
  ['retryable: error.retryable', 'stable boundary retryability'],
]) requireText(boundaryMapper, value, label);

for (const [value, label] of [
  ['self.compensate_payment(tenant_id, actor_id, operation)', 'payment-first orchestration'],
  ['self.compensate_order(tenant_id, actor_id, operation)', 'order-second orchestration'],
  ['self.release_remaining_reservations(tenant_id, operation)', 'inventory-third orchestration'],
  ['self.release_cart(tenant_id, operation)', 'cart-fourth orchestration'],
]) requireText(orchestrationBlock, value, label);

for (const [value, label] of [
  ['read_cart_checkout_snapshot(', 'cart snapshot read remains mounted'],
  ['release_cart_checkout(', 'cart release remains mounted'],
]) requireText(cartBlock, value, label);

for (const [value, label] of [
  [
    'release_inventory_by_identity(\n                            inventory_context(',
    'inline inventory context delegation',
  ],
  ['.map_err(|error| boundary_error("release_inventory", error))?', 'context-dropping inventory mapper'],
  ['owner_boundary_error("release_inventory", error)', 'legacy inventory owner mapper'],
]) forbidText(inventoryBlock, value, label);

if (failures.length > 0) {
  console.error('Checkout compensation inventory context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout inventory compensation retains owner context without changing payment/order behavior or public envelopes',
);
