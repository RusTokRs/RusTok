#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const lib = read('crates/rustok-cart/src/lib.rs');
const guard = read('crates/rustok-cart/src/atomic_checkout_guard.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(lib, 'mod atomic_checkout_guard;', 'atomic checkout guard registration');
forbidText(
  lib,
  'pub use atomic_checkout_port::*;',
  'legacy atomic checkout wildcard export',
);
requireText(
  lib,
  'pub use atomic_checkout_guard::{',
  'guarded atomic checkout top-level export',
);
for (const value of [
  'AtomicCartCheckoutBinding',
  'AtomicCartCheckoutHandle',
  'bind_in_process_atomic_cart_checkout',
  'bind_in_process_atomic_cart_checkout_with_pricing',
  'in_process_atomic_cart_checkout_port',
]) {
  requireText(lib, value, 'guarded atomic checkout export');
}
requireText(
  lib,
  'pub mod atomic_checkout_port;',
  'legacy atomic checkout module compatibility',
);
requireText(
  lib,
  'pub use atomic_checkout_port::{',
  'legacy atomic checkout type compatibility',
);

for (const [value, label] of [
  [
    'const READ_ATOMIC_CART_CHECKOUT_SNAPSHOT_OPERATION',
    'atomic checkout read operation',
  ],
  [
    'const UPDATE_ATOMIC_CART_CHECKOUT_CONTEXT_OPERATION',
    'atomic checkout context operation',
  ],
  ['const BEGIN_ATOMIC_CART_CHECKOUT_OPERATION', 'atomic checkout begin operation'],
  [
    'const RELEASE_ATOMIC_CART_CHECKOUT_OPERATION',
    'atomic checkout release operation',
  ],
  [
    'const COMPLETE_ATOMIC_CART_CHECKOUT_OPERATION',
    'atomic checkout complete operation',
  ],
  [
    'const PREPARE_ATOMIC_CART_CHECKOUT_OPERATION',
    'atomic checkout handle prepare operation',
  ],
  [
    'legacy::bind_in_process_atomic_cart_checkout(',
    'legacy atomic checkout binding delegation',
  ],
  [
    'legacy::bind_in_process_atomic_cart_checkout_with_pricing(',
    'legacy priced atomic checkout binding delegation',
  ],
  ['wrap_binding(', 'guarded atomic checkout binding cutover'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation = owner_operation', 'owner operation logging'],
  ['owner_code = %error.code', 'internal owner code logging'],
  ['owner_kind = ?error.kind', 'internal owner kind logging'],
  [
    'PortActor::service("rustok-cart.atomic-checkout-guard")',
    'synthetic handle owner context',
  ],
  [
    'map_atomic_checkout_error(',
    'atomic checkout public error sanitization',
  ],
  [
    '"cart checkout pricing changed; retry with a fresh cart snapshot"',
    'stable pricing change message',
  ],
  [
    '"cart checkout adapter does not match the requested cart"',
    'stable cart mismatch message',
  ],
  [
    '"cart checkout request context is invalid"',
    'stable tenant context message',
  ],
  [
    '"cart checkout could not acquire the required cart lock"',
    'stable checkout lock message',
  ],
  [
    '"cart checkout prepared state is unavailable"',
    'stable prepared state message',
  ],
  ['"cart checkout request is invalid"', 'stable validation message'],
  ['"cart was not found"', 'stable cart not-found message'],
  ['"cart line item was not found"', 'stable line-item not-found message'],
  [
    '"cart lifecycle transition conflicts with the current state"',
    'stable lifecycle message',
  ],
  [
    '"cart checkout storage is temporarily unavailable"',
    'stable storage message',
  ],
  [
    '"cart checkout operation could not be completed safely"',
    'stable fallback message',
  ],
]) {
  requireText(guard, value, label);
}

for (const value of [
  'error.message',
  'format!("checkout adapter is bound to cart',
  'format!("cart {cart_id} not found")',
  'format!("cart line item {line_item_id} not found")',
  'format!("invalid cart status transition:',
  'PortError::new(kind, code, message, retryable)',
]) {
  forbidText(guard, value, 'atomic checkout public error guard');
}

if (failures.length > 0) {
  console.error('Atomic cart checkout error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Top-level atomic cart checkout bindings preserve the legacy implementation behind correlation-aware stable public errors',
);
