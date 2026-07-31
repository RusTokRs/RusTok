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
  ports: 'crates/rustok-inventory/src/ports.rs',
  error: 'crates/rustok-commerce-foundation/src/error.rs',
  evidence:
    'crates/rustok-inventory/contracts/evidence/inventory-port-diagnostic-safety-source.json',
  review:
    'crates/rustok-inventory/contracts/evidence/inventory-port-diagnostic-safety-source-review.json',
  document: 'crates/rustok-inventory/docs/inventory-port-diagnostic-safety.md',
  broad: 'scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs',
  broadTest: 'scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs',
};

const ports = read(paths.ports);
const errorSource = read(paths.error);
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

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
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
  failures.push(`unterminated function ${functionName}`);
  return '';
}

for (const marker of [
  'pub trait InventoryReservationPort: Send + Sync',
  'pub trait InventoryReservationIdentityPort: Send + Sync',
  'async fn check_availability(',
  'async fn reserve_inventory(',
  'async fn release_inventory_reservation(',
  'async fn reserve_inventory_by_identity(',
  'async fn release_inventory_by_identity(',
  'let owner_operation = "check_availability";',
  'let owner_operation = "reserve_inventory";',
  'let owner_operation = "release_inventory_reservation";',
  'let owner_operation = "reserve_inventory_by_identity";',
  'let owner_operation = "release_inventory_by_identity";',
  'context.require_policy(PortCallPolicy::read())?;',
  'context.require_policy(PortCallPolicy::write())?;',
  'context.require_write_semantics()?;',
  'parse_port_tenant_id(&context, owner_operation)?;',
  'inventory_error_to_port_error(&context, owner_operation, error)',
  'storage_unavailable_with_context(&context, owner_operation, error)',
  'storage_unavailable_with_context(context, owner_operation, error)',
  'self.check_variant_availability_for_channel(',
  'self.reserve(tenant_id, request.variant_id, request.quantity)',
  'self.release_reservation_quantity(tenant_id, request.variant_id, request.quantity)',
  'self.db.begin().await.map_err(|error|',
  '.lock_exclusive()',
  'reservation_item::ActiveModel',
  'inventory_level::Entity::update_many()',
]) requireText(ports, marker, `${paths.ports}: preserved port behavior`);

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

const contextFacts = functionBody(ports, 'inventory_port_context_facts');
const errorFacts = functionBody(ports, 'inventory_owner_error_facts');
const ownerLogger = functionBody(ports, 'log_inventory_port_failure');
const tenantLogger = functionBody(ports, 'log_inventory_tenant_parse_rejection');
const tenantParser = functionBody(ports, 'parse_port_tenant_id');
const storageMapper = functionBody(ports, 'storage_unavailable_with_context');
const variantLookup = functionBody(ports, 'load_tenant_variant');
const mapper = functionBody(ports, 'inventory_error_to_port_error');
const diagnosticScope = [
  contextFacts,
  errorFacts,
  ownerLogger,
  tenantLogger,
  tenantParser,
  storageMapper,
  variantLookup,
  mapper,
].join('\n');

for (const marker of [
  'const INVENTORY_PORT_BOUNDARY: &str = "inventory_reservation_port";',
  'struct InventoryPortContextFacts',
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_kind',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
  'struct InventoryOwnerErrorFacts',
  "error_variant: &'static str",
  'text_field_count: usize',
  'text_total_length: usize',
  'uuid_field_count: usize',
  'uuid_non_nil_count: usize',
  'numeric_field_count: usize',
  'numeric_nonzero_count: usize',
  'numeric_negative_count: usize',
  'opaque_payload_present: bool',
]) requireText(ports, marker, `${paths.ports}: bounded fact shape`);

for (const marker of [
  'CommerceError::Database(_) => InventoryOwnerErrorFacts::opaque("database")',
  'CommerceError::ProductNotFound(value)',
  'InventoryOwnerErrorFacts::uuid("product_not_found", *value)',
  'CommerceError::VariantNotFound(value)',
  'InventoryOwnerErrorFacts::uuid("variant_not_found", *value)',
  'CommerceError::DuplicateHandle { handle, locale }',
  '..InventoryOwnerErrorFacts::empty("duplicate_handle")',
  'CommerceError::DuplicateSku(value)',
  '..InventoryOwnerErrorFacts::empty("duplicate_sku")',
  'CommerceError::InvalidPrice(value)',
  '..InventoryOwnerErrorFacts::empty("invalid_price")',
  'CommerceError::InsufficientInventory {',
  'numeric_field_count: 2',
  'numeric_nonzero_count:',
  'numeric_negative_count:',
  '..InventoryOwnerErrorFacts::empty("insufficient_inventory")',
  'CommerceError::InvalidOptionCombination',
  'InventoryOwnerErrorFacts::empty("invalid_option_combination")',
  'CommerceError::Validation(value)',
  '..InventoryOwnerErrorFacts::empty("validation")',
  'CommerceError::ShippingProfileNotFound(value)',
  'InventoryOwnerErrorFacts::uuid("shipping_profile_not_found", *value)',
  'CommerceError::DuplicateShippingProfileSlug(value)',
  '..InventoryOwnerErrorFacts::empty("duplicate_shipping_profile_slug")',
  'CommerceError::NoVariants => InventoryOwnerErrorFacts::empty("no_variants")',
  'CommerceError::CannotDeletePublished',
  'InventoryOwnerErrorFacts::empty("cannot_delete_published")',
  'CommerceError::Rich(_) => InventoryOwnerErrorFacts::opaque("rich")',
  'CommerceError::Core(_) => InventoryOwnerErrorFacts::opaque("core")',
]) requireText(errorFacts, marker, `${paths.ports}: exhaustive bounded CommerceError facts`);

for (const marker of [
  'tracing::error!(',
  'tracing::warn!(',
  'owner = "rustok_inventory"',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'actor_id_length = context_facts.actor_id_length',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'channel_present = context_facts.channel_present',
  'locale_length = context_facts.locale_length',
  'operation = owner_operation',
  'code,',
  'error_variant = error_facts.error_variant',
  'text_field_count = error_facts.text_field_count',
  'text_total_length = error_facts.text_total_length',
  'uuid_field_count = error_facts.uuid_field_count',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'numeric_field_count = error_facts.numeric_field_count',
  'numeric_nonzero_count = error_facts.numeric_nonzero_count',
  'numeric_negative_count = error_facts.numeric_negative_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = INVENTORY_PORT_BOUNDARY',
]) requireText(ownerLogger, marker, `${paths.ports}: bounded owner logger`);

for (const marker of [
  'Uuid::parse_str(context.tenant_id.trim()).map_err(|_|',
  'log_inventory_tenant_parse_rejection(context, owner_operation);',
  '"inventory.context_invalid"',
  '"inventory request context is invalid"',
]) requireText(tenantParser, marker, `${paths.ports}: stable tenant parser`);
for (const marker of [
  'tracing::warn!(',
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id_parse_failed = true',
  'code = "inventory.context_invalid"',
  'boundary = INVENTORY_PORT_BOUNDARY',
]) requireText(tenantLogger, marker, `${paths.ports}: bounded tenant parser logger`);

for (const marker of [
  '_error: sea_orm::DbErr',
  'InventoryOwnerErrorFacts::opaque("database")',
  '"inventory.database_unavailable"',
  '&facts',
  'true',
  '"inventory storage is temporarily unavailable"',
]) requireText(storageMapper, marker, `${paths.ports}: bounded storage mapper`);

for (const marker of [
  'InventoryOwnerErrorFacts::uuid("variant_not_found", variant_id)',
  '"inventory.variant_not_found"',
  '&facts',
  'false',
  '"inventory variant was not found"',
]) requireText(variantLookup, marker, `${paths.ports}: bounded variant lookup`);

for (const [variant, code, message, constructor, technical] of [
  [
    'CommerceError::Database(_)',
    'inventory.database_unavailable',
    'inventory storage is temporarily unavailable',
    'PortError::unavailable',
    'true',
  ],
  [
    'CommerceError::VariantNotFound(_)',
    'inventory.variant_not_found',
    'inventory variant was not found',
    'PortError::new',
    'false',
  ],
  [
    'CommerceError::InsufficientInventory { .. }',
    'inventory.insufficient_inventory',
    'inventory reservation conflicts with available stock',
    'PortError::new',
    'false',
  ],
  [
    'CommerceError::InvalidPrice(_) | CommerceError::Validation(_)',
    'inventory.validation',
    'inventory request is invalid',
    'PortError::validation',
    'false',
  ],
  [
    '_ =>',
    'inventory.invariant_violation',
    'inventory operation violated an owner invariant',
    'PortError::invariant_violation',
    'true',
  ],
]) {
  for (const marker of [variant, `"${code}"`, `"${message}"`, constructor]) {
    requireText(mapper, marker, `${paths.ports}: stable ${code} mapping`);
  }
  const call = new RegExp(
    `"${code.replaceAll('.', '\\.')}",[\\s\\S]*?&error_facts,[\\s\\S]*?${technical},`,
  );
  if (!call.test(mapper)) {
    failures.push(`${paths.ports}: ${code} severity classification drift`);
  }
}

for (const forbidden of [
  'error = ?error',
  'error = ?other',
  'error = %error',
  'internal_message = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'variant_id = %variant_id',
  'product_id = %',
  'shipping_profile_id = %',
  'handle = %',
  'sku = %',
  'slug = %',
  'requested =',
  'available =',
]) forbidText(diagnosticScope, forbidden, `${paths.ports}: inventory payload diagnostics`);

for (const marker of [
  "[inventory, 'tenant_id_length = context_facts.tenant_id_length', 'inventory tenant shape logging']",
  "[inventory, 'tenant_id_parse_failed = true', 'inventory tenant parse failure']",
  "[inventory, 'error_variant = error_facts.error_variant', 'inventory static error variant']",
  "[inventory, 'numeric_field_count = error_facts.numeric_field_count', 'inventory numeric error shape']",
  "[inventory, 'boundary = INVENTORY_PORT_BOUNDARY', 'inventory port boundary']",
]) requireText(broad, marker, `${paths.broad}: aggregate Inventory coverage`);

for (const marker of [
  'function canonicalCustomer()',
  'tenant_id_length = context_facts.tenant_id_length',
  'tenant_id_parse_failed = true',
  'function canonicalInventory()',
  'numeric_field_count = error_facts.numeric_field_count',
  'inventory payload diagnostics: forbidden',
]) requireText(broadTest, marker, `${paths.broadTest}: aggregate fixture coverage`);

for (const [key, expected] of Object.entries({
  commerce_error_variant_count: 15,
  complete_commerce_error_logged: false,
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
  tenant_parser_diagnostic_cleanup_closed: true,
  storage_mapper_diagnostic_cleanup_closed: true,
  variant_lookup_diagnostic_cleanup_closed: true,
  owner_mapper_diagnostic_cleanup_closed: true,
  inventory_ports_boundary_diagnostic_cleanup_closed: true,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  technical_error_severity_changed: false,
  ordinary_warning_severity_changed: false,
  port_operations_changed: false,
  owner_delegation_changed: false,
  sql_or_state_machine_changed: false,
  request_response_contract_changed: false,
  reservation_owner_context_cleanup_closed: false,
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
  all_five_port_operations_preserved: true,
  public_request_response_contracts_preserved: true,
  owner_delegation_preserved: true,
  sql_and_reservation_state_machine_preserved: true,
  public_error_mappings_preserved: true,
  technical_error_severity_preserved: true,
  ordinary_warning_severity_preserved: true,
  all_fifteen_commerce_error_variants_classified: true,
  complete_commerce_error_logging_removed: true,
  database_rich_core_payload_removed: true,
  raw_context_removed: true,
  raw_uuid_removed: true,
  validation_price_and_identity_text_removed: true,
  exact_inventory_counts_removed: true,
  bounded_context_shape_retained: true,
  bounded_owner_error_shape_retained: true,
  tenant_parser_payload_removed: true,
  storage_mapper_payload_removed: true,
  variant_lookup_payload_removed: true,
  inventory_ports_boundary_source_closed: true,
  reservation_owner_context_remains_separate: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Inventory port diagnostic safety',
  'Status: **source-ready / unvalidated**',
  'tenant UUID parsing',
  'tenant-scoped variant lookup',
  'SeaORM storage-error mapping',
  '`inventory_error_to_port_error`',
  'All fifteen current `CommerceError` variants',
  'The database payload is treated as opaque and is not logged.',
  '`crates/rustok-inventory/src/reservation_owner_context.rs`',
  'The broader ecommerce mapper cleanup also remains open.',
]) requireText(document, marker, `${paths.document}: truthful source scope`);

if (failures.length > 0) {
  console.error('Inventory port diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Inventory tenant parsing, variant lookup, storage mapping and CommerceError mapping retain bounded diagnostics while preserving public and reservation behavior; execution evidence remains open',
);
