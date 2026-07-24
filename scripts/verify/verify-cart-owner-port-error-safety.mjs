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
const guarded = read('crates/rustok-cart/src/guarded_ports.rs');
const owner = read('crates/rustok-cart/src/owner_ports.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(lib, 'mod owner_ports;', 'cart owner module registration');
forbidText(
  guarded,
  'crate::ports::in_process_cart_storefront_port',
  'guarded storefront legacy delegation',
);
forbidText(
  guarded,
  'crate::ports::in_process_cart_checkout_port',
  'guarded checkout legacy delegation',
);
requireText(
  guarded,
  'crate::owner_ports::owner_cart_storefront_port',
  'guarded storefront owner delegation',
);
requireText(
  guarded,
  'crate::owner_ports::owner_cart_checkout_port',
  'guarded checkout owner delegation',
);

for (const [value, label] of [
  ['const READ_CART_CHECKOUT_SNAPSHOT_OPERATION', 'checkout snapshot operation'],
  ['const UPDATE_CART_CHECKOUT_CONTEXT_OPERATION', 'checkout context operation'],
  ['const BEGIN_CART_CHECKOUT_OPERATION', 'checkout begin operation'],
  ['const RELEASE_CART_CHECKOUT_OPERATION', 'checkout release operation'],
  ['const COMPLETE_CART_CHECKOUT_OPERATION', 'checkout completion operation'],
  ['const READ_STOREFRONT_CART_OPERATION', 'storefront read operation'],
  ['const CREATE_STOREFRONT_CART_OPERATION', 'storefront create operation'],
  ['const ADD_STOREFRONT_LINE_ITEM_OPERATION', 'storefront add-line operation'],
  ['const UPDATE_STOREFRONT_CONTEXT_OPERATION', 'storefront context operation'],
  [
    'const UPDATE_STOREFRONT_LINE_ITEM_QUANTITY_OPERATION',
    'storefront quantity operation',
  ],
  [
    'const UPDATE_STOREFRONT_LINE_ITEM_PRICING_OPERATION',
    'storefront pricing operation',
  ],
  ['const REMOVE_STOREFRONT_LINE_ITEM_OPERATION', 'storefront remove operation'],
  ['const REPRICE_STOREFRONT_LINE_ITEMS_OPERATION', 'storefront reprice operation'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation = owner_operation', 'owner operation logging'],
  ['code = "cart.context_invalid"', 'context stable code'],
  ['code = "cart.tenant_id_invalid"', 'tenant stable code'],
  ['"cart request context is invalid"', 'stable context message'],
  ['"cart request is invalid"', 'stable validation message'],
  ['"cart was not found"', 'stable cart not-found message'],
  ['"cart line item was not found"', 'stable line-item not-found message'],
  [
    '"cart lifecycle transition conflicts with the current state"',
    'stable lifecycle message',
  ],
  ['"cart storage is temporarily unavailable"', 'stable storage message'],
  ['"cart tax recalculation failed"', 'stable tax-boundary message'],
  [
    'cart_error_to_port_error(&context, owner_operation, error)',
    'context-aware owner error mapping',
  ],
  [
    'parse_cart_tenant_id(&context, owner_operation)',
    'context-aware tenant mapping',
  ],
  [
    'require_cart_policy(&context, owner_operation, PortCallPolicy::read())',
    'context-aware read policy',
  ],
  [
    'require_cart_policy(&context, owner_operation, PortCallPolicy::write())',
    'context-aware write policy',
  ],
]) {
  requireText(owner, value, label);
}

for (const value of [
  '.map_err(cart_error_to_port_error)',
  'fn cart_error_to_port_error(error: CartError)',
  'format!("cart {id} not found")',
  'format!("cart line item {id} not found")',
  'format!("invalid cart status transition: {from} -> {to}")',
  'format!("cart storage unavailable: {error}")',
  'PortError::validation("cart.validation", message)',
  'PortError::new(kind, code, message, retryable)',
]) {
  forbidText(owner, value, 'cart owner public error mapping');
}

if (failures.length > 0) {
  console.error('Cart owner port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Guarded cart storefront and checkout ports use correlation-aware owner mapping while preserving guest access and stable public errors',
);
