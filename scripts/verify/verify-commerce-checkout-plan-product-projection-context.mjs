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

const inventoryValidationBlock = between(
  source,
  '    async fn validate_cart_inventory(',
  '    async fn validate_delivery_groups(',
  'checkout inventory and projection validation block',
);
const productContextBlock = between(
  source,
  'fn product_context(',
  'fn inventory_context(',
  'product projection context helper',
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
  'fn checkout_plan_product_projection_boundary_error(',
  'marketplace snapshot boundary mapper',
);
const productMapper = between(
  source,
  'fn checkout_plan_product_projection_boundary_error(',
  'fn checkout_plan_inventory_boundary_error(',
  'product projection boundary mapper',
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
    'const CHECKOUT_PLAN_PRODUCT_PROJECTION_BOUNDARY: &str =\n    "commerce_checkout_plan_product_projection";',
    'stable product projection boundary identity',
  ],
  ['const PRODUCT_PROJECTION_OWNER: &str = "rustok_product";', 'truthful product owner'],
  [
    'const PRODUCT_PROJECTION_READ_OPERATION: &str = "read_product_projection";',
    'exact product projection operation',
  ],
  [
    'const VARIANT_PRODUCT_PROJECTION_READ_OPERATION: &str = "read_variant_product_projection";',
    'exact variant projection operation',
  ],
]) requireText(source, value, label);

for (const [value, label] of [
  ['let product_context =', 'retained product context'],
  [
    'product_context(tenant_id, actor_id, cart, public_channel_slug.as_deref())',
    'product context construction',
  ],
  ['.read_product_projection(', 'product projection owner call'],
  ['.read_variant_product_projection(', 'variant projection owner call'],
  ['product_context.clone()', 'product context delegation clone'],
  ['ProductProjectionRequest {', 'product projection request'],
  ['product_id,', 'product identity'],
  ['VariantProductProjectionRequest {', 'variant projection request'],
  ['variant_id,', 'variant identity'],
  ['locale: cart.locale_code.clone()', 'projection locale'],
  ['fallback_locale: None', 'unchanged projection fallback'],
  ['&product_context', 'product mapper context input'],
  ['PRODUCT_PROJECTION_READ_OPERATION', 'product owner operation selection'],
  ['VARIANT_PRODUCT_PROJECTION_READ_OPERATION', 'variant owner operation selection'],
  [
    'checkout_plan_product_projection_boundary_error(',
    'context-aware product projection mapper',
  ],
  ['ProductStatus::Active', 'product active validation'],
  ['product.published_at.is_none()', 'product publication validation'],
  ['is_metadata_visible_for_public_channel(', 'product channel visibility validation'],
  ['.find(|variant| variant.id == variant_id)', 'variant membership validation'],
  [
    'current_shipping_profile_slug != line_item.shipping_profile_slug',
    'shipping profile snapshot validation',
  ],
  ['checkout_plan_inventory_boundary_error(&inventory_context, error)', 'inventory mapper remains context-aware'],
]) requireText(inventoryValidationBlock, value, label);

const contextBindings = inventoryValidationBlock.match(/let product_context\s*=/g) ?? [];
const contextClones = inventoryValidationBlock.match(/product_context\.clone\(\)/g) ?? [];
const mapperInputs = inventoryValidationBlock.match(/&product_context/g) ?? [];
const productOperations = inventoryValidationBlock.match(/PRODUCT_PROJECTION_READ_OPERATION/g) ?? [];
const variantOperations = inventoryValidationBlock.match(/VARIANT_PRODUCT_PROJECTION_READ_OPERATION/g) ?? [];
if (
  contextBindings.length !== 1 ||
  contextClones.length !== 2 ||
  mapperInputs.length !== 2 ||
  productOperations.length !== 1 ||
  variantOperations.length !== 1
) {
  failures.push(
    `expected one retained product context, two branch clones, two mapper inputs, and one exact operation per branch, found ${contextBindings.length}/${contextClones.length}/${mapperInputs.length}/${productOperations.length}/${variantOperations.length}`,
  );
}

for (const [value, label] of [
  ['port_context(tenant_id, actor_id, cart, channel_slug, "product")', 'product correlation boundary selection'],
]) requireText(productContextBlock, value, label);

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
  ["owner_operation: &'static str", 'exact owner operation input'],
  ['error: PortError', 'original port error input'],
  [
    'log_checkout_plan_product_projection_boundary_failure(context, owner_operation, &error);',
    'diagnostics before mapping',
  ],
  [
    'boundary_error("read_checkout_product_projection", error)',
    'unchanged public stage mapping',
  ],
  ['fn log_checkout_plan_product_projection_boundary_failure(', 'structured diagnostic helper'],
  ['error = ?error', 'original port error'],
  ['owner = PRODUCT_PROJECTION_OWNER', 'truthful owner field'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation field'],
  ['stage = "read_checkout_product_projection"', 'commerce stage field'],
  ['code = %error.code', 'port code'],
  ['internal_message = %error.message', 'public-safe port message'],
  ['error_kind = ?error.kind', 'typed port error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = CHECKOUT_PLAN_PRODUCT_PROJECTION_BOUNDARY', 'boundary field'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical severity classification',
  ],
  ['tracing::error!(', 'technical failure severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['"checkout plan product projection owner boundary failed"', 'technical event'],
  ['"checkout plan product projection owner boundary was rejected"', 'rejection event'],
]) requireText(productMapper, value, label);

const diagnosticsIndex = productMapper.indexOf(
  'log_checkout_plan_product_projection_boundary_failure(context, owner_operation, &error);',
);
const mappingIndex = productMapper.indexOf(
  'boundary_error("read_checkout_product_projection", error)',
);
if (!(diagnosticsIndex >= 0 && diagnosticsIndex < mappingIndex)) {
  failures.push('product projection diagnostics must run before public boundary mapping');
}

for (const [content, value, label] of [
  [marketplaceMapper, 'checkout_plan_marketplace_snapshot_boundary_error(', 'marketplace mapper remains context-aware'],
  [marketplaceMapper, 'log_checkout_plan_marketplace_snapshot_boundary_failure(context, &error);', 'marketplace diagnostics remain mounted'],
  [inventoryMapper, 'checkout_plan_inventory_boundary_error(', 'inventory mapper remains context-aware'],
  [inventoryMapper, 'log_checkout_plan_inventory_boundary_failure(context, &error);', 'inventory diagnostics remain mounted'],
]) requireText(content, value, label);

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
    '.read_product_projection(\n                            product_context(',
    'inline product projection context delegation',
  ],
  [
    '.read_variant_product_projection(\n                            product_context(',
    'inline variant projection context delegation',
  ],
  [
    '.map_err(|error| boundary_error("read_checkout_product_projection", error))?',
    'context-dropping shared product mapper',
  ],
  [
    'checkout_plan_product_projection_boundary_error(error)',
    'context-free product projection mapper',
  ],
]) forbidText(inventoryValidationBlock, value, label);

if (failures.length > 0) {
  console.error('Checkout plan product projection context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Checkout plan product and variant projection reads retain product-owner context without changing marketplace, inventory, validation, or public envelope behavior',
);
