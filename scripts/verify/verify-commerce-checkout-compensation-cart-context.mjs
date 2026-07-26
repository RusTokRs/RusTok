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
const cartStatusBlock = between(
  source,
  'fn cart_status(',
  'fn inventory_context(',
  'cart status helper',
);
const cartContextBlock = between(
  source,
  'fn cart_context(',
  'fn order_context(',
  'cart context helper',
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
  ['const CART_COMPENSATION_OWNER: &str = "rustok_cart";', 'truthful cart owner'],
  [
    'const CART_SNAPSHOT_OPERATION: &str = "read_cart_checkout_snapshot";',
    'exact cart snapshot operation',
  ],
  [
    'const CART_RELEASE_OPERATION: &str = "release_cart_checkout";',
    'exact cart release operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let cart_read_context =', 'retained cart read context'],
  ['cart_context(tenant_id, operation, self.port_deadline, "read", false)', 'cart read context construction'],
  ['cart_read_context.clone()', 'cart read context delegation clone'],
  ['&cart_read_context', 'cart read context mapper input'],
  ['CART_COMPENSATION_OWNER', 'cart read mapper owner'],
  ['CART_SNAPSHOT_OPERATION', 'cart snapshot mapper operation'],
  ['"read_cart"', 'cart read commerce stage'],
  ['CartCheckoutSnapshotRequest {', 'cart snapshot request'],
  ['cart_id: operation.cart_id', 'cart snapshot identity'],
  ['locale: None', 'cart snapshot locale request'],
  ['let cart_release_context =', 'retained cart release context'],
  ['cart_context(tenant_id, operation, self.port_deadline, "release", true)', 'cart release context construction'],
  ['cart_release_context.clone()', 'cart release context delegation clone'],
  ['&cart_release_context', 'cart release context mapper input'],
  ['CART_RELEASE_OPERATION', 'cart release mapper operation'],
  ['"release_cart"', 'cart release commerce stage'],
  ['CartCheckoutLifecycleRequest {', 'cart release request'],
  ['CartStatus::CheckingOut', 'checking-out release admission'],
  ['CartStatus::Active => {}', 'active cart no-op'],
  ['CartStatus::Completed', 'completed cart reconciliation'],
  ['CartStatus::Abandoned', 'abandoned cart conflict'],
  ['cart_status(&released)? != CartStatus::Active', 'released cart active validation'],
]) requireText(cartBlock, value, label);

const readBindings = cartBlock.match(/let cart_read_context\s*=/g) ?? [];
const readClones = cartBlock.match(/cart_read_context\.clone\(\)/g) ?? [];
const readMapperInputs = cartBlock.match(/&cart_read_context/g) ?? [];
const releaseBindings = cartBlock.match(/let cart_release_context\s*=/g) ?? [];
const releaseClones = cartBlock.match(/cart_release_context\.clone\(\)/g) ?? [];
const releaseMapperInputs = cartBlock.match(/&cart_release_context/g) ?? [];
if (readBindings.length !== 1 || readClones.length !== 1 || readMapperInputs.length !== 1) {
  failures.push(
    `expected one retained cart read context, one clone, and one mapper input, found ${readBindings.length}/${readClones.length}/${readMapperInputs.length}`,
  );
}
if (
  releaseBindings.length !== 1 ||
  releaseClones.length !== 1 ||
  releaseMapperInputs.length !== 1
) {
  failures.push(
    `expected one retained cart release context, one clone, and one mapper input, found ${releaseBindings.length}/${releaseClones.length}/${releaseMapperInputs.length}`,
  );
}

for (const [value, label] of [
  ['PortActor::service("rustok-commerce.checkout-compensation")', 'cart service actor'],
  ['PLATFORM_FALLBACK_LOCALE', 'cart effective locale'],
  ['checkout:{}:compensation:cart:{action}', 'cart correlation identity'],
  ['.with_causation_id(operation.id.to_string())', 'cart causation'],
  ['.with_deadline(deadline)', 'cart deadline'],
  ['if write {', 'cart write semantics branch'],
  ['.with_idempotency_key(format!(', 'cart release idempotency key'],
]) requireText(cartContextBlock, value, label);

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
  failures.push('cart owner diagnostics must precede unchanged owner routing and boundary mapping');
}

for (const [value, label] of [
  ['ORDER_MANUAL_RECONCILIATION_CODE | PAYMENT_MANUAL_RECONCILIATION_CODE', 'manual routing remains payment/order only'],
  ['boundary_error(stage, error)', 'cart generic boundary delegation'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['CheckoutCompensationError::Boundary {', 'stable boundary envelope'],
  ['stage,', 'stable boundary stage'],
  ['code: error.code', 'stable boundary code'],
  ['message: error.message', 'stable boundary message'],
  ['retryable: error.retryable', 'stable boundary retryability'],
]) requireText(boundaryMapper, value, label);

for (const [value, label] of [
  ['cart.lifecycle_status()', 'typed cart lifecycle conversion'],
  ['cart {} has an unknown lifecycle state', 'unknown lifecycle reconciliation'],
]) requireText(cartStatusBlock, value, label);

for (const [value, label] of [
  ['self.compensate_payment(tenant_id, actor_id, operation)', 'payment-first orchestration'],
  ['self.compensate_order(tenant_id, actor_id, operation)', 'order-second orchestration'],
  ['self.release_remaining_reservations(tenant_id, operation)', 'inventory-third orchestration'],
  ['self.release_cart(tenant_id, operation)', 'cart-fourth orchestration'],
]) requireText(orchestrationBlock, value, label);

for (const [content, value, label] of [
  [paymentBlock, 'compensate_checkout_payment(', 'payment compensation remains mounted'],
  [orderBlock, 'compensate_checkout_order(', 'order compensation remains mounted'],
  [inventoryBlock, 'release_inventory_by_identity(', 'inventory release remains mounted'],
]) requireText(content, value, label);

for (const [value, label] of [
  [
    'read_cart_checkout_snapshot(\n                cart_context(tenant_id, operation, self.port_deadline, "read", false)',
    'inline cart read context delegation',
  ],
  [
    'release_cart_checkout(\n                        cart_context(tenant_id, operation, self.port_deadline, "release", true)',
    'inline cart release context delegation',
  ],
  ['.map_err(|error| boundary_error("read_cart", error))?', 'context-dropping cart read mapper'],
  ['.map_err(|error| boundary_error("release_cart", error))?', 'context-dropping cart release mapper'],
  ['owner_boundary_error("read_cart", error)', 'legacy cart read owner mapper'],
  ['owner_boundary_error("release_cart", error)', 'legacy cart release owner mapper'],
]) forbidText(cartBlock, value, label);

if (failures.length > 0) {
  console.error('Checkout compensation cart context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout cart compensation retains snapshot and release owner context without changing public envelopes',
);
