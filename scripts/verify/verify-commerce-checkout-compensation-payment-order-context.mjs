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

const paymentBlock = between(
  source,
  '    async fn compensate_payment(',
  '    async fn compensate_order(',
  'payment compensation block',
);
const orderBlock = between(
  source,
  '    async fn compensate_order(',
  '    async fn release_remaining_reservations(',
  'order compensation block',
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
const orchestrationBlock = between(
  source,
  '    async fn compensate_claimed(',
  '    async fn compensate_payment(',
  'compensation orchestration block',
);

for (const [value, label] of [
  ['PortErrorKind', 'typed port error classification import'],
  [
    'const CHECKOUT_COMPENSATION_OWNER_BOUNDARY: &str = "checkout_compensation_owner_port";',
    'stable owner boundary identity',
  ],
  ['const PAYMENT_COMPENSATION_OWNER: &str = "rustok_payment";', 'truthful payment owner'],
  ['const ORDER_COMPENSATION_OWNER: &str = "rustok_order";', 'truthful order owner'],
  [
    'const PAYMENT_COMPENSATION_OPERATION: &str = "compensate_checkout_payment";',
    'exact payment owner operation',
  ],
  [
    'const ORDER_COMPENSATION_OPERATION: &str = "compensate_checkout_order";',
    'exact order owner operation',
  ],
]) requireText(source, value, label);

for (const [content, value, label] of [
  [
    paymentBlock,
    'let payment_context =\n            payment_context(tenant_id, actor_id, operation, self.port_deadline);',
    'retained payment context',
  ],
  [paymentBlock, 'payment_context.clone()', 'payment context delegation clone'],
  [paymentBlock, '&payment_context', 'payment context mapper input'],
  [paymentBlock, 'PAYMENT_COMPENSATION_OWNER', 'payment mapper owner'],
  [paymentBlock, 'PAYMENT_COMPENSATION_OPERATION', 'payment mapper operation'],
  [paymentBlock, '"compensate_payment"', 'payment commerce stage'],
  [orderBlock, 'let order_context = order_context(', 'retained order context'],
  [orderBlock, 'order_context.clone()', 'order context delegation clone'],
  [orderBlock, '&order_context', 'order context mapper input'],
  [orderBlock, 'ORDER_COMPENSATION_OWNER', 'order mapper owner'],
  [orderBlock, 'ORDER_COMPENSATION_OPERATION', 'order mapper operation'],
  [orderBlock, '"compensate_order"', 'order commerce stage'],
]) requireText(content, value, label);

const paymentContextBindings = paymentBlock.match(/let payment_context\s*=/g) ?? [];
const paymentContextClones = paymentBlock.match(/payment_context\.clone\(\)/g) ?? [];
const orderContextBindings = orderBlock.match(/let order_context\s*=/g) ?? [];
const orderContextClones = orderBlock.match(/order_context\.clone\(\)/g) ?? [];
if (paymentContextBindings.length !== 1 || paymentContextClones.length !== 1) {
  failures.push(
    `expected one retained payment context and one clone, found ${paymentContextBindings.length}/${paymentContextClones.length}`,
  );
}
if (orderContextBindings.length !== 1 || orderContextClones.length !== 1) {
  failures.push(
    `expected one retained order context and one clone, found ${orderContextBindings.length}/${orderContextClones.length}`,
  );
}

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
  ['"checkout compensation owner call failed"', 'technical diagnostic event'],
  ['"checkout compensation owner call was rejected"', 'rejection diagnostic event'],
]) requireText(ownerMapper, value, label);

const diagnosticIndex = ownerMapper.indexOf(
  'log_owner_boundary_error(context, owner, operation, stage, &error);',
);
const manualRoutingIndex = ownerMapper.indexOf('if matches!(');
if (!(diagnosticIndex >= 0 && diagnosticIndex < manualRoutingIndex)) {
  failures.push('owner diagnostics must run before manual-reconciliation/boundary mapping');
}

for (const [value, label] of [
  ['ORDER_MANUAL_RECONCILIATION_CODE | PAYMENT_MANUAL_RECONCILIATION_CODE', 'manual routing codes'],
  ['CheckoutCompensationError::ManualReconciliation(error.message)', 'manual reconciliation envelope'],
  ['boundary_error(stage, error)', 'generic boundary delegation'],
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
  ['self.release_remaining_reservations(tenant_id, operation)', 'inventory release orchestration'],
  ['self.release_cart(tenant_id, operation)', 'cart release orchestration'],
  ['CheckoutOperationStage::PaymentCaptured', 'captured-funds admission'],
]) requireText(orchestrationBlock, value, label);

for (const [content, value, label] of [
  [paymentBlock, 'PaymentCollectionStatusKind::Cancelled', 'payment cancelled validation'],
  [
    paymentBlock,
    'payment owner returned no compensation result',
    'payment missing-result manual reconciliation',
  ],
  [orderBlock, 'OrderStatusKind::Cancelled', 'order cancelled validation'],
  [orderBlock, 'order owner returned no compensation result', 'order missing-result reconciliation'],
  [
    inventoryBlock,
    '.map_err(|error| boundary_error("release_inventory", error))?',
    'inventory release remains out of scope',
  ],
  [
    cartBlock,
    '.map_err(|error| boundary_error("read_cart", error))?',
    'cart read remains out of scope',
  ],
  [
    cartBlock,
    '.map_err(|error| boundary_error("release_cart", error))?',
    'cart release remains out of scope',
  ],
]) requireText(content, value, label);

for (const [value, label] of [
  [
    'compensate_checkout_payment(\n                payment_context(tenant_id, actor_id, operation, self.port_deadline)',
    'inline payment context delegation',
  ],
  [
    'compensate_checkout_order(\n                order_context(tenant_id, actor_id, operation, self.port_deadline)',
    'inline order context delegation',
  ],
  ['owner_boundary_error("compensate_payment", error)', 'context-dropping payment mapper'],
  ['owner_boundary_error("compensate_order", error)', 'context-dropping order mapper'],
]) forbidText(source, value, label);

if (failures.length > 0) {
  console.error('Checkout compensation payment/order context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout payment/order compensation retain owner context without changing inventory/cart paths or public envelopes',
);
