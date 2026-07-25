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
const portContract = read('crates/rustok-api/src/ports.rs');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
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

const targetValidation = between(
  guard,
  'fn validate_cart_promotion_request(',
  'fn parse_cart_promotion_tenant_id(',
  'promotion target validation',
);
const tenantParser = between(
  guard,
  'fn parse_cart_promotion_tenant_id(',
  'fn cart_promotion_context_error(',
  'promotion tenant parser',
);
const contextMapper = between(
  guard,
  'fn cart_promotion_context_error(',
  'fn cart_promotion_error(',
  'promotion context mapper',
);
const ownerMapper = between(
  guard,
  'fn cart_promotion_error(',
  'fn cart_promotion_error_code(',
  'promotion owner mapper',
);

for (const [value, label] of [
  ['const CART_PROMOTION_OWNER: &str = "rustok_cart.promotion";', 'promotion owner constant'],
  ['const READ_CART_PROMOTION_PREVIEW_OPERATION', 'preview owner operation'],
  ['const APPLY_CART_PROMOTION_OPERATION', 'apply owner operation'],
  ['service: CartService::new(db)', 'direct owner service construction'],
  [
    'cart_promotion_context_error(&context, owner_operation, error)',
    'context-aware policy mapping',
  ],
  [
    'cart_promotion_error(&context, owner_operation, error)',
    'context-aware owner error mapping',
  ],
  [
    'validate_cart_promotion_request(&context, owner_operation, &request)',
    'context-aware target validation',
  ],
  [
    'parse_cart_promotion_tenant_id(&context, owner_operation)',
    'context-aware tenant parsing',
  ],
]) {
  requireText(guard, value, label);
}

for (const [source, label] of [
  [targetValidation, 'promotion target validation'],
  [tenantParser, 'promotion tenant parser'],
  [contextMapper, 'promotion context mapper'],
  [ownerMapper, 'promotion owner mapper'],
]) {
  for (const [value, detail] of [
    ['owner = CART_PROMOTION_OWNER', `${label} owner log`],
    ['correlation_id = %context.correlation_id', `${label} correlation log`],
    ['tenant_id = %context.tenant_id', `${label} tenant log`],
    ['channel = ?context.channel', `${label} channel log`],
    ['operation = owner_operation', `${label} operation log`],
  ]) {
    requireText(source, value, detail);
  }
}

for (const [source, value, label] of [
  [targetValidation, 'scope = ?request.scope', 'promotion target scope detail'],
  [targetValidation, 'line_item_present = request.line_item_id.is_some()', 'promotion target line detail'],
  [tenantParser, 'error = ?error', 'promotion tenant parse cause'],
  [tenantParser, 'internal_tenant_id = %context.tenant_id', 'promotion tenant internal identity'],
  [contextMapper, 'internal_code = %error.code', 'promotion context internal code'],
  [contextMapper, 'internal_message = %error.message', 'promotion context internal message'],
  [contextMapper, 'kind = ?error.kind', 'promotion context kind'],
  [contextMapper, 'retryable = error.retryable', 'promotion context retryability'],
  [ownerMapper, 'error = ?error', 'promotion raw owner cause'],
  [ownerMapper, 'let code = cart_promotion_error_code(&error);', 'promotion stable code selection'],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['code = "cart.tenant_id_invalid"', 'tenant stable code'],
  ['code = "cart.promotion_context_invalid"', 'context stable code'],
  ['"cart promotion request context is invalid"', 'stable context message'],
  ['"cart promotion request is invalid"', 'stable validation message'],
  ['"cart was not found"', 'stable cart not-found message'],
  ['"cart line item was not found"', 'stable line-item not-found message'],
  [
    '"cart promotion conflicts with the current cart state"',
    'stable state conflict message',
  ],
  ['"cart promotion tax recalculation failed"', 'stable tax-boundary message'],
  ['"cart storage is temporarily unavailable"', 'stable storage message'],
]) {
  requireText(guard, value, label);
}

for (const value of [
  'crate::ports::in_process_cart_promotion_port(db)',
  '.map_err(cart_error_to_port_error)',
  'PortError::validation("cart.validation", message)',
  'format!("cart storage unavailable: {error}")',
  'format!("cart {id} not found")',
  'format!("cart line item {id} not found")',
]) {
  forbidText(guard, value, 'promotion public error mapping');
}

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub struct PortError {', 'shared port error'],
]) {
  requireText(portContract, value, label);
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
  '✔ Cart promotion preview/apply retain owner, channel, correlation, and stable public error envelopes',
);
