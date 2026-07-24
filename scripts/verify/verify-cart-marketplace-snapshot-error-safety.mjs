#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-cart/src/marketplace_snapshot.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  '.map_err(map_cart_error)',
  'fn map_cart_error(error: CartError)',
  'fn parse_tenant_id(context: &PortContext)',
  '"marketplace cart snapshot requires a UUID tenant_id"',
  'PortError::new(\n            PortErrorKind::Conflict,\n            "cart.marketplace_snapshot_conflict",\n            message,',
  'format!("cart {cart_id} not found")',
  'format!("cart line item {line_item_id} not found")',
  'format!("invalid cart status transition: {from} -> {to}")',
  'PortError::new(kind, code, message, retryable)',
]) {
  forbidText(value, 'marketplace cart snapshot public error mapping');
}

for (const [value, label] of [
  [
    'const LIST_MARKETPLACE_LINE_SNAPSHOTS_OPERATION',
    'marketplace snapshot list operation',
  ],
  [
    'const ADD_MARKETPLACE_LINE_ITEM_OPERATION',
    'marketplace snapshot add operation',
  ],
  [
    'const BIND_MARKETPLACE_LINE_SNAPSHOT_OPERATION',
    'marketplace snapshot bind operation',
  ],
  [
    'let owner_operation = LIST_MARKETPLACE_LINE_SNAPSHOTS_OPERATION;',
    'marketplace snapshot list mapping',
  ],
  [
    'let owner_operation = ADD_MARKETPLACE_LINE_ITEM_OPERATION;',
    'marketplace snapshot add mapping',
  ],
  [
    'let owner_operation = BIND_MARKETPLACE_LINE_SNAPSHOT_OPERATION;',
    'marketplace snapshot bind mapping',
  ],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id = %context.tenant_id', 'tenant logging'],
  ['operation = owner_operation', 'owner operation logging'],
  [
    'code = "cart.marketplace_snapshot_context_invalid"',
    'context stable code',
  ],
  [
    'code = "cart.marketplace_snapshot_tenant_invalid"',
    'tenant stable code',
  ],
  [
    '"marketplace cart snapshot request context is invalid"',
    'stable context message',
  ],
  [
    '"marketplace cart snapshot conflicts with the current cart state"',
    'stable conflict message',
  ],
  ['"cart was not found"', 'stable cart not-found message'],
  ['"cart line item was not found"', 'stable line-item not-found message'],
  [
    '"cart lifecycle transition conflicts with the current state"',
    'stable lifecycle message',
  ],
  [
    '"marketplace cart snapshot storage is temporarily unavailable"',
    'stable storage message',
  ],
  [
    '"marketplace cart snapshot tax recalculation failed"',
    'stable tax-boundary message',
  ],
  [
    'require_marketplace_snapshot_policy(\n            &context,\n            owner_operation,\n            PortCallPolicy::read(),',
    'context-aware read policy',
  ],
  [
    'require_marketplace_snapshot_policy(\n            &context,\n            owner_operation,\n            PortCallPolicy::write(),',
    'context-aware write policy',
  ],
  [
    'parse_tenant_id(&context, owner_operation)',
    'context-aware tenant mapping',
  ],
  [
    'map_cart_error(&context, owner_operation, error)',
    'context-aware owner mapping',
  ],
]) {
  requireText(value, label);
}

if (failures.length > 0) {
  console.error('Marketplace cart snapshot error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Marketplace cart snapshot ports keep identifying and technical owner failures in correlation-aware logs and expose stable public errors',
);
