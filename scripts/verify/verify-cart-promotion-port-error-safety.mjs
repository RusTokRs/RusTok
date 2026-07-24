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

const lib = read('crates/rustok-cart/src/lib.rs');
const guard = read('crates/rustok-cart/src/promotion_guard.rs');
const ports = read('crates/rustok-cart/src/ports.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(lib, 'mod promotion_guard;', 'promotion guard module');
requireText(
  lib,
  'pub use promotion_guard::guarded_cart_promotion_port as in_process_cart_promotion_port;',
  'top-level promotion constructor cutover',
);

const portExports = lib.match(/pub use ports::\{([\s\S]*?)\n\};/)?.[1] ?? '';
forbidText(
  portExports,
  'in_process_cart_promotion_port',
  'legacy constructor top-level export',
);

for (const [value, label] of [
  ['const READ_CART_PROMOTION_PREVIEW_OPERATION', 'preview owner operation'],
  ['const APPLY_CART_PROMOTION_OPERATION', 'apply owner operation'],
  [
    'crate::ports::in_process_cart_promotion_port(db)',
    'internal compatibility delegation',
  ],
  [
    'cart_promotion_port_error(&context, owner_operation, error)',
    'context-aware error mapping',
  ],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation = owner_operation', 'owner operation logging'],
  ['internal_code = %error.code', 'internal code logging'],
  ['internal_message = %error.message', 'internal message logging'],
  ['"cart promotion request context is invalid"', 'stable context message'],
  ['"cart promotion request is invalid"', 'stable validation message'],
  ['"cart was not found"', 'stable cart not-found message'],
  ['"cart line item was not found"', 'stable line-item not-found message'],
  [
    '"cart promotion conflicts with the current cart state"',
    'stable state conflict message',
  ],
  ['"cart promotion tax recalculation failed"', 'stable tax-boundary message'],
  ['"cart promotion is temporarily unavailable"', 'stable unavailable message'],
  ['"cart promotion could not be completed safely"', 'stable invariant message'],
]) {
  requireText(guard, value, label);
}

for (const value of [
  'return error;',
  'PortError::new(kind, code, message, retryable)',
  '.map_err(|error| error)',
]) {
  forbidText(guard, value, 'promotion public error mapping');
}

requireText(
  ports,
  'impl CartPromotionPort for crate::CartService',
  'legacy promotion provider compatibility',
);

if (failures.length > 0) {
  console.error('Cart promotion port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Cart promotion preview/apply use the correlation-aware safe wrapper and keep internal owner errors out of the public contract',
);
