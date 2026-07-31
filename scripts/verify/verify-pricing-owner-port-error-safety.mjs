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

const paths = {
  ports: 'crates/rustok-pricing/src/ports.rs',
  error: 'crates/rustok-commerce-foundation/src/error.rs',
  readContext: 'crates/rustok-pricing/src/read_context.rs',
  writeContext: 'crates/rustok-pricing/src/write_context.rs',
  evidence:
    'crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source.json',
  review:
    'crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source-review.json',
  document: 'crates/rustok-pricing/docs/pricing-owner-port-error-safety.md',
  broad: 'scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs',
  broadTest: 'scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs',
};

const ports = read(paths.ports);
const errorSource = read(paths.error);
const readContext = read(paths.readContext);
const writeContext = read(paths.writeContext);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);
const broad = read(paths.broad);
const broadTest = read(paths.broadTest);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
}

function functionBody(source, name) {
  const match = new RegExp(`(?:async\\s+)?fn\\s+${name}\\s*\\(`).exec(source);
  if (!match) {
    failures.push(`missing function ${name}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${name}`);
  return '';
}

const readImpl = between(
  ports,
  'impl PricingReadPort for crate::PricingService {',
  '#[async_trait]\nimpl PricingWritePort for crate::PricingService {',
  'pricing read implementation',
);
const writeImpl = between(
  ports,
  'impl PricingWritePort for crate::PricingService {',
  'struct PricingPortContextFacts {',
  'pricing write implementation',
);

for (const [operation, kind] of [
  ['resolve_product_price', 'read'],
  ['read_price_list_projection', 'read'],
  ['list_active_price_list_projections', 'read'],
  ['read_admin_product_pricing_projection', 'read'],
  ['read_storefront_product_pricing_projection', 'read'],
  ['preview_variant_discount', 'read'],
  ['upsert_variant_price', 'write'],
  ['set_price_list_scope', 'write'],
  ['apply_variant_discount', 'write'],
  ['set_price_list_percentage_rule', 'write'],
]) {
  const body = functionBody(kind === 'read' ? readImpl : writeImpl, operation);
  requireText(body, `let owner_operation = "${operation}";`, `${operation} operation`);
  requireText(
    body,
    'parse_port_tenant_id(&context, owner_operation)?;',
    `${operation} tenant parsing`,
  );
  if (kind === 'write') {
    requireText(
      body,
      'parse_port_actor_id(&context, owner_operation)?;',
      `${operation} actor parsing`,
    );
  }
}

for (const marker of [
  'pub trait PricingReadPort: Send + Sync',
  'pub trait PricingWritePort: Send + Sync',
  'context.require_policy(PortCallPolicy::read())?;',
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'self.resolve_variant_price(',
  'self.list_active_price_lists(',
  'self.list_active_price_lists_for_channel(',
  'self.get_admin_product_pricing_with_locale_fallback(',
  'self.get_published_product_pricing_by_handle_with_locale_fallback(',
  'self.upsert_admin_variant_price_with_channel(',
  'self.set_price_list_scope(',
  'self.apply_percentage_discount_with_channel(',
  'self.set_price_list_percentage_rule_projection(',
]) requireText(ports, marker, `${paths.ports}: preserved behavior`);

for (const marker of [
  'Database(#[from] sea_orm::DbErr)',
  'ProductNotFound(Uuid)',
  'VariantNotFound(Uuid)',
  'DuplicateHandle { handle: String, locale: String }',
  'DuplicateSku(String)',
  'InvalidPrice(String)',
  'InsufficientInventory { requested: i32, available: i32 }',
  'InvalidOptionCombination',
  'Validation(String)',
  'ShippingProfileNotFound(Uuid)',
  'DuplicateShippingProfileSlug(String)',
  'NoVariants',
  'CannotDeletePublished',
  'Rich(#[source] Box<RichError>)',
  'Core(#[from] CoreError)',
]) requireText(errorSource, marker, `${paths.error}: CommerceError shape`);

const contextFacts = functionBody(ports, 'pricing_port_context_facts');
const errorFacts = functionBody(ports, 'pricing_owner_error_facts');
const ownerLogger = functionBody(ports, 'log_pricing_port_failure');
const parserLogger = functionBody(ports, 'log_pricing_context_rejection');
const tenantParser = functionBody(ports, 'parse_port_tenant_id');
const actorParser = functionBody(ports, 'parse_port_actor_id');
const mapper = functionBody(ports, 'pricing_error_to_port_error');
const resolve = functionBody(readImpl, 'resolve_product_price');
const readList = functionBody(readImpl, 'read_price_list_projection');
const diagnosticScope = [
  contextFacts,
  errorFacts,
  ownerLogger,
  parserLogger,
  tenantParser,
  actorParser,
  mapper,
  resolve,
  readList,
].join('\n');

for (const marker of [
  'const PRICING_OWNER: &str = "rustok_pricing";',
  'const PRICING_PORT_BOUNDARY: &str = "pricing_owner_port";',
  'struct PricingPortContextFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
  'struct PricingOwnerErrorFacts',
  "error_variant: &'static str",
  'text_field_count: usize',
  'text_total_length: usize',
  'uuid_field_count: usize',
  'uuid_non_nil_count: usize',
  'numeric_field_count: usize',
  'numeric_nonzero_count: usize',
  'numeric_negative_count: usize',
  'opaque_payload_present: bool',
]) requireText(ports, marker, `${paths.ports}: bounded facts`);

for (const marker of [
  'PricingOwnerErrorFacts::opaque("database")',
  'PricingOwnerErrorFacts::uuids("product_not_found"',
  'PricingOwnerErrorFacts::uuids("variant_not_found"',
  'PricingOwnerErrorFacts::text("duplicate_handle"',
  'PricingOwnerErrorFacts::text("duplicate_sku"',
  'PricingOwnerErrorFacts::text("invalid_price"',
  'PricingOwnerErrorFacts::numbers(',
  '"insufficient_inventory"',
  'PricingOwnerErrorFacts::empty("invalid_option_combination")',
  'PricingOwnerErrorFacts::text("validation"',
  'PricingOwnerErrorFacts::uuids("shipping_profile_not_found"',
  '"duplicate_shipping_profile_slug"',
  'PricingOwnerErrorFacts::empty("no_variants")',
  'PricingOwnerErrorFacts::empty("cannot_delete_published")',
  'PricingOwnerErrorFacts::opaque("rich")',
  'PricingOwnerErrorFacts::opaque("core")',
]) requireText(errorFacts, marker, `${paths.ports}: exhaustive error facts`);

for (const marker of [
  'tracing::error!(',
  'tracing::warn!(',
  'owner = PRICING_OWNER',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'operation,',
  'code,',
  'error_variant = error_facts.error_variant',
  'text_field_count = error_facts.text_field_count',
  'uuid_field_count = error_facts.uuid_field_count',
  'numeric_field_count = error_facts.numeric_field_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = PRICING_PORT_BOUNDARY',
]) requireText(ownerLogger, marker, `${paths.ports}: bounded logger`);

for (const marker of [
  'tracing::warn!(',
  'parse_target,',
  'parse_failed = true',
  'boundary = PRICING_PORT_BOUNDARY',
]) requireText(parserLogger, marker, `${paths.ports}: parser logger`);
for (const [parser, code, message, target] of [
  [tenantParser, 'pricing.tenant_id_invalid', 'pricing request context is invalid', 'tenant_id'],
  [actorParser, 'pricing.actor_id_invalid', 'pricing write actor is invalid', 'actor_id'],
]) {
  for (const marker of [`"${code}"`, `"${message}"`, `"${target}"`]) {
    requireText(parser, marker, `${paths.ports}: parser outcome`);
  }
}

for (const [source, code, message] of [
  [resolve, 'pricing.variant_product_mismatch', 'variant does not belong to the requested product'],
  [resolve, 'pricing.price_not_found', 'price was not found'],
  [readList, 'pricing.price_list_not_found', 'price list was not found'],
]) {
  for (const marker of [`"${code}"`, `"${message}"`, 'PricingOwnerErrorFacts::uuids(']) {
    requireText(source, marker, `${paths.ports}: direct outcome`);
  }
}

for (const [variant, code, message, technical] of [
  ['CommerceError::Database(_)', 'pricing.database_unavailable', 'pricing storage is temporarily unavailable', 'true'],
  ['CommerceError::ProductNotFound(_)', 'pricing.product_not_found', 'product was not found', 'false'],
  ['CommerceError::VariantNotFound(_)', 'pricing.variant_not_found', 'variant was not found', 'false'],
  ['CommerceError::DuplicateHandle { .. }', 'pricing.duplicate_handle', 'pricing handle is already in use', 'false'],
  ['CommerceError::DuplicateSku(_)', 'pricing.duplicate_sku', 'pricing SKU is already in use', 'false'],
  ['CommerceError::InvalidPrice(_) | CommerceError::Validation(_)', 'pricing.validation', 'pricing request is invalid', 'false'],
  ['CommerceError::InsufficientInventory { .. }', 'pricing.insufficient_inventory', 'inventory is insufficient for the pricing operation', 'false'],
  ['CommerceError::InvalidOptionCombination', 'pricing.invalid_option_combination', 'invalid option combination', 'false'],
  ['CommerceError::ShippingProfileNotFound(_)', 'pricing.shipping_profile_not_found', 'shipping profile was not found', 'false'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'pricing.duplicate_shipping_profile_slug', 'shipping profile slug is already in use', 'false'],
  ['CommerceError::NoVariants', 'pricing.no_variants', 'product must have at least one variant', 'false'],
  ['CommerceError::CannotDeletePublished', 'pricing.cannot_delete_published', 'cannot delete published product', 'false'],
  ['CommerceError::Rich(_)', 'pricing.rich_error', 'pricing operation failed an internal invariant', 'true'],
  ['CommerceError::Core(_)', 'pricing.core_error', 'pricing operation failed an internal invariant', 'true'],
]) {
  for (const marker of [variant, `"${code}"`, `"${message}"`]) {
    requireText(mapper, marker, `${paths.ports}: ${code} mapping`);
  }
  const severity = new RegExp(
    `"${code.replaceAll('.', '\\.')}",[\\s\\S]*?&error_facts,[\\s\\S]*?${technical},`,
  );
  if (!severity.test(mapper)) failures.push(`${paths.ports}: ${code} severity drift`);
}

for (const forbidden of [
  'format!(',
  'error = ?error',
  'cause = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'product_id = %',
  'variant_id = %',
  'price_list_id = %',
  'handle = %',
  'sku = %',
  'slug = %',
  'PortContext.tenant_id must be a UUID for pricing ports',
  'pricing write actor must be a UUID',
]) forbidText(diagnosticScope, forbidden, `${paths.ports}: payload diagnostics`);

for (const marker of [
  'tenant_id_length = context_facts.tenant_id_length',
  'parse_failed = true',
  'error_variant = error_facts.error_variant',
  'numeric_field_count = error_facts.numeric_field_count',
  'boundary = PRICING_PORT_BOUNDARY',
  'pricing payload diagnostics',
]) requireText(broad, marker, `${paths.broad}: Pricing coverage`);
for (const marker of [
  'function canonicalPricing()',
  'struct PricingPortContextFacts {}',
  'tenant_id_length = context_facts.tenant_id_length',
  'parse_failed = true',
  'numeric_field_count = error_facts.numeric_field_count',
  'pricing payload diagnostics: forbidden',
]) requireText(broadTest, marker, `${paths.broadTest}: Pricing fixture`);

for (const [key, expected] of Object.entries({
  port_operation_count: 10,
  read_operation_count: 6,
  write_operation_count: 4,
  commerce_error_variant_count: 15,
  complete_owner_error_logged: false,
  database_error_payload_logged: false,
  rich_error_payload_logged: false,
  core_error_payload_logged: false,
  raw_context_logged: false,
  raw_uuid_logged: false,
  validation_or_price_text_logged: false,
  handle_sku_slug_text_logged: false,
  exact_inventory_counts_logged: false,
  static_error_variant_logged: true,
  aggregate_text_shape_logged: true,
  aggregate_uuid_shape_logged: true,
  aggregate_numeric_shape_logged: true,
  opaque_payload_presence_logged: true,
  bounded_context_shape_logged: true,
  tenant_parser_cleanup_closed: true,
  actor_parser_cleanup_closed: true,
  direct_lookup_message_cleanup_closed: true,
  owner_mapper_cleanup_closed: true,
  pricing_ports_boundary_cleanup_closed: true,
  public_code_changed: false,
  public_message_changed: true,
  public_kind_changed: false,
  public_retryability_changed: false,
  technical_error_severity_changed: false,
  ordinary_warning_severity_changed: false,
  port_operations_changed: false,
  owner_delegation_changed: false,
  pricing_business_logic_changed: false,
  request_response_contract_changed: false,
  read_context_wrapper_cleanup_closed: false,
  write_context_wrapper_cleanup_closed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push(`${paths.evidence}: execution must remain empty`);
}

for (const [key, expected] of Object.entries({
  all_ten_port_operations_preserved: true,
  public_request_response_contracts_preserved: true,
  owner_delegation_preserved: true,
  pricing_business_logic_preserved: true,
  public_error_codes_kinds_retryability_preserved: true,
  dynamic_public_messages_replaced_with_static_messages: true,
  technical_error_severity_preserved: true,
  ordinary_warning_severity_preserved: true,
  all_fifteen_commerce_error_variants_classified: true,
  complete_owner_error_logging_removed: true,
  database_rich_core_payload_removed: true,
  raw_context_removed: true,
  raw_uuid_removed: true,
  validation_price_and_identity_text_removed: true,
  exact_inventory_counts_removed: true,
  bounded_context_shape_retained: true,
  bounded_owner_error_shape_retained: true,
  tenant_parser_payload_removed: true,
  actor_parser_payload_removed: true,
  direct_lookup_payload_removed: true,
  pricing_ports_boundary_source_closed: true,
  read_context_wrapper_remains_separate: true,
  write_context_wrapper_remains_separate: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Pricing owner port error safety',
  'Status: **source-ready / unvalidated**',
  'tenant and write-actor UUID parsing',
  '`pricing_error_to_port_error`',
  'All fifteen current `CommerceError` variants',
  '`crates/rustok-pricing/src/read_context.rs`',
  '`crates/rustok-pricing/src/write_context.rs`',
  'The broader ecommerce mapper cleanup also remains open.',
]) requireText(document, marker, `${paths.document}: truthful scope`);
requireText(
  readContext,
  'const PRICING_READ_BOUNDARY: &str = "pricing_read_port";',
  'read wrapper remains separate',
);
requireText(
  writeContext,
  'const PRICING_WRITE_BOUNDARY: &str = "pricing_write_port";',
  'write wrapper remains separate',
);

if (failures.length > 0) {
  console.error('Pricing owner port error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Pricing ports retain ten owner operations and static public outcomes with bounded context/error shape; wrappers and execution evidence remain open',
);
