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
const inventoryBlock = between(
  source,
  '    async fn validate_cart_inventory(',
  '    async fn validate_delivery_groups(',
  'checkout inventory validation block',
);
const inventoryContextBlock = between(
  source,
  'fn inventory_context(',
  'fn port_context(',
  'inventory context helper',
);
const portContextBlock = between(
  source,
  'fn port_context(',
  'fn checkout_plan_inventory_boundary_error(',
  'shared checkout plan context helper',
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
    'const CHECKOUT_PLAN_INVENTORY_BOUNDARY: &str = "commerce_checkout_plan_inventory";',
    'stable inventory boundary identity',
  ],
  ['const INVENTORY_OWNER: &str = "rustok_inventory";', 'truthful inventory owner'],
  [
    'const INVENTORY_AVAILABILITY_OPERATION: &str = "check_availability";',
    'exact inventory owner operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let inventory_context =', 'retained inventory context'],
  [
    'inventory_context(tenant_id, actor_id, cart, public_channel_slug.as_deref())',
    'inventory context construction',
  ],
  ['inventory_context.clone()', 'inventory context delegation clone'],
  ['&inventory_context', 'inventory mapper context input'],
  ['.check_availability(', 'inventory owner call'],
  ['InventoryAvailabilityRequest {', 'inventory request'],
  ['variant_id,', 'variant identity'],
  ['requested_quantity: line_item.quantity', 'requested quantity'],
  ['channel_slug: public_channel_slug.clone()', 'normalized channel request'],
  [
    'checkout_plan_inventory_boundary_error(&inventory_context, error)',
    'context-aware inventory mapper',
  ],
  ['"Variant {variant_id} does not have enough available inventory for the cart channel"', 'insufficient inventory validation'],
  ['read_checkout_product_projection', 'product projection behavior remains mounted'],
  ['ProductStatus::Active', 'product active validation'],
  ['current_shipping_profile_slug != line_item.shipping_profile_slug', 'shipping profile snapshot validation'],
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
  ['port_context(tenant_id, actor_id, cart, channel_slug, "inventory")', 'inventory correlation boundary selection'],
]) requireText(inventoryContextBlock, value, label);

for (const [value, label] of [
  ['normalize_locale_tag', 'locale normalization'],
  ['PLATFORM_FALLBACK_LOCALE.to_string()', 'fallback locale'],
  ['PortActor::user(actor_id.to_string())', 'checkout actor'],
  ['format!("checkout:{}:{boundary}", cart.id)', 'cart correlation identity'],
  ['.with_deadline(Duration::from_secs(2))', 'owner deadline'],
  ['Some(channel_slug) => context.with_channel(channel_slug)', 'normalized channel context'],
]) requireText(portContextBlock, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'retained context input'],
  ['error: PortError', 'original port error input'],
  ['log_checkout_plan_inventory_boundary_failure(context, &error);', 'diagnostics before mapping'],
  ['boundary_error("check_inventory_availability", error)', 'unchanged public stage mapping'],
  ['fn log_checkout_plan_inventory_boundary_failure(', 'structured diagnostic helper'],
  ['error = ?error', 'original port error'],
  ['owner = INVENTORY_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = INVENTORY_AVAILABILITY_OPERATION', 'exact owner operation field'],
  ['stage = "check_inventory_availability"', 'commerce stage field'],
  ['code = %error.code', 'port code'],
  ['internal_message = %error.message', 'public-safe port message'],
  ['error_kind = ?error.kind', 'typed port error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = CHECKOUT_PLAN_INVENTORY_BOUNDARY', 'boundary field'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical failure severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"checkout plan inventory owner boundary failed"', 'technical event'],
  ['"checkout plan inventory owner boundary was rejected"', 'rejection event'],
]) requireText(inventoryMapper, value, label);

const diagnosticsIndex = inventoryMapper.indexOf(
  'log_checkout_plan_inventory_boundary_failure(context, &error);',
);
const mappingIndex = inventoryMapper.indexOf(
  'boundary_error("check_inventory_availability", error)',
);
if (!(diagnosticsIndex >= 0 && diagnosticsIndex < mappingIndex)) {
  failures.push('inventory diagnostics must run before public boundary mapping');
}

for (const [value, label] of [
  ['CheckoutError::BoundaryFailure {', 'stable public boundary envelope'],
  ['stage,', 'stable stage'],
  ['kind: error.kind', 'stable typed kind'],
  ['code: error.code', 'stable code'],
  ['message: error.message', 'stable message'],
  ['retryable: error.retryable', 'stable retryability'],
]) requireText(genericMapper, value, label);

for (const [value, label] of [
  ['list_marketplace_line_snapshots(', 'marketplace snapshot remains mounted'],
  ['boundary_error("read_marketplace_cart_snapshots", error)', 'marketplace mapper remains out of scope'],
]) requireText(buildBlock, value, label);

for (const [value, label] of [
  [
    '.check_availability(\n                    inventory_context(',
    'inline inventory context delegation',
  ],
  [
    '.map_err(|error| boundary_error("check_inventory_availability", error))?',
    'context-dropping inventory mapper',
  ],
  ['checkout_plan_inventory_boundary_error(error)', 'context-free inventory mapper'],
]) forbidText(inventoryBlock, value, label);

if (failures.length > 0) {
  console.error('Checkout plan inventory context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout plan inventory availability retains owner context without changing product, marketplace, validation, or public envelope behavior',
);
