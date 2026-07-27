#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/services/checkout_plan_builder.rs', root),
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

const buildBlock = between(
  source,
  '    pub async fn build(',
  '    async fn validate_cart_inventory(',
  'checkout plan build block',
);
const marketplaceContextBlock = between(
  source,
  'fn marketplace_snapshot_context(',
  'fn product_context(',
  'marketplace snapshot context helper',
);
const sharedContextBlock = between(
  source,
  'fn port_context(',
  'fn checkout_plan_marketplace_snapshot_boundary_error(',
  'shared checkout plan context helper',
);
const marketplaceMapper = between(
  source,
  'fn checkout_plan_marketplace_snapshot_boundary_error(',
  'fn checkout_plan_inventory_boundary_error(',
  'marketplace snapshot boundary mapper',
);
const inventoryMapper = between(
  source,
  'fn checkout_plan_inventory_boundary_error(',
  'fn boundary_error(',
  'inventory boundary mapper',
);
const genericMapper = between(
  source,
  'fn boundary_error(',
  'fn stage_error',
  'generic checkout boundary mapper',
);

for (const [value, label] of [
  ['PortErrorKind', 'typed port error classification import'],
  [
    'const CHECKOUT_PLAN_MARKETPLACE_SNAPSHOT_BOUNDARY: &str =\n    "commerce_checkout_plan_marketplace_snapshot";',
    'stable marketplace snapshot boundary identity',
  ],
  ['const MARKETPLACE_SNAPSHOT_OWNER: &str = "rustok_cart";', 'truthful cart owner'],
  [
    'const MARKETPLACE_SNAPSHOT_OPERATION: &str = "list_marketplace_line_snapshots";',
    'exact marketplace snapshot owner operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let marketplace_snapshot_context =', 'retained marketplace snapshot context'],
  ['marketplace_snapshot_context(', 'marketplace snapshot context construction'],
  ['normalize_public_channel_slug(cart.channel_slug.as_deref()).as_deref()', 'normalized channel context input'],
  ['.list_marketplace_line_snapshots(', 'marketplace snapshot owner call'],
  ['marketplace_snapshot_context.clone()', 'marketplace snapshot context delegation clone'],
  ['ListMarketplaceCartLineSnapshotsRequest { cart_id: cart.id }', 'cart snapshot request identity'],
  ['&marketplace_snapshot_context', 'marketplace snapshot mapper context input'],
  [
    'checkout_plan_marketplace_snapshot_boundary_error(',
    'context-aware marketplace snapshot mapper',
  ],
  ['build_marketplace_plan_lines(cart, marketplace_snapshots)', 'typed marketplace plan construction'],
  ['line.snapshot.seller_id.to_string()', 'typed marketplace seller projection'],
  ['self.validate_cart_inventory(tenant_id, actor_id, cart)', 'inventory validation remains mounted'],
]) requireText(buildBlock, value, label);

const contextBindings = buildBlock.match(/let marketplace_snapshot_context\s*=/g) ?? [];
const contextClones = buildBlock.match(/marketplace_snapshot_context\.clone\(\)/g) ?? [];
const mapperInputs = buildBlock.match(/&marketplace_snapshot_context/g) ?? [];
if (contextBindings.length !== 1 || contextClones.length !== 1 || mapperInputs.length !== 1) {
  failures.push(
    `expected one retained marketplace snapshot context, one clone, and one mapper input, found ${contextBindings.length}/${contextClones.length}/${mapperInputs.length}`,
  );
}

for (const [value, label] of [
  ['port_context(', 'shared context delegation'],
  ['tenant_id,', 'tenant context input'],
  ['actor_id,', 'actor context input'],
  ['cart,', 'cart context input'],
  ['channel_slug,', 'channel context input'],
  ['"marketplace-snapshot"', 'marketplace correlation boundary selection'],
]) requireText(marketplaceContextBlock, value, label);

for (const [value, label] of [
  ['normalize_locale_tag', 'locale normalization'],
  ['PLATFORM_FALLBACK_LOCALE.to_string()', 'fallback locale'],
  ['PortActor::user(actor_id.to_string())', 'checkout actor'],
  ['format!("checkout:{}:{boundary}", cart.id)', 'cart correlation identity'],
  ['.with_deadline(Duration::from_secs(2))', 'owner deadline'],
  ['Some(channel_slug) => context.with_channel(channel_slug)', 'normalized channel context'],
]) requireText(sharedContextBlock, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ['error: PortError', 'original port error input'],
  [
    'log_checkout_plan_marketplace_snapshot_boundary_failure(context, &error);',
    'diagnostics before mapping',
  ],
  [
    'boundary_error("read_marketplace_cart_snapshots", error)',
    'unchanged public stage mapping',
  ],
  ['fn log_checkout_plan_marketplace_snapshot_boundary_failure(', 'structured diagnostic helper'],
  ['error = ?error', 'original port error'],
  ['owner = MARKETPLACE_SNAPSHOT_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = MARKETPLACE_SNAPSHOT_OPERATION', 'exact owner operation field'],
  ['stage = "read_marketplace_cart_snapshots"', 'commerce stage field'],
  ['code = %error.code', 'port code'],
  ['internal_message = %error.message', 'public-safe port message'],
  ['error_kind = ?error.kind', 'typed port error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = CHECKOUT_PLAN_MARKETPLACE_SNAPSHOT_BOUNDARY', 'boundary field'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical failure severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"checkout plan marketplace snapshot owner boundary failed"', 'technical event'],
  ['"checkout plan marketplace snapshot owner boundary was rejected"', 'rejection event'],
]) requireText(marketplaceMapper, value, label);

const diagnosticsIndex = marketplaceMapper.indexOf(
  'log_checkout_plan_marketplace_snapshot_boundary_failure(context, &error);',
);
const mappingIndex = marketplaceMapper.indexOf(
  'boundary_error("read_marketplace_cart_snapshots", error)',
);
if (!(diagnosticsIndex >= 0 && diagnosticsIndex < mappingIndex)) {
  failures.push('marketplace snapshot diagnostics must run before public boundary mapping');
}

for (const [value, label] of [
  ['checkout_plan_inventory_boundary_error(', 'inventory mapper remains context-aware'],
  ['log_checkout_plan_inventory_boundary_failure(context, &error);', 'inventory diagnostics remain mounted'],
  ['boundary_error("check_inventory_availability", error)', 'inventory public mapping remains stable'],
]) requireText(inventoryMapper, value, label);

for (const [value, label] of [
  ['CheckoutError::BoundaryFailure {', 'stable public boundary envelope'],
  ['stage,', 'stable stage'],
  ['kind: error.kind', 'stable typed kind'],
  ['code: error.code', 'stable code'],
  ['message: error.message', 'stable message'],
  ['retryable: error.retryable', 'stable retryability'],
]) requireText(genericMapper, value, label);

for (const [value, label] of [
  [
    '.list_marketplace_line_snapshots(\n                port_context(',
    'inline marketplace snapshot context delegation',
  ],
  [
    '.map_err(|error| boundary_error("read_marketplace_cart_snapshots", error))?',
    'context-dropping marketplace snapshot mapper',
  ],
  [
    'checkout_plan_marketplace_snapshot_boundary_error(error)',
    'context-free marketplace snapshot mapper',
  ],
]) forbidText(buildBlock, value, label);

if (failures.length > 0) {
  console.error('Checkout plan marketplace snapshot context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout plan marketplace snapshot reads retain cart-owner context without changing inventory or public envelope behavior',
);
