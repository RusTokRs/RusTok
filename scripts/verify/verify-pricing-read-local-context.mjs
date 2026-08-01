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
  lib: 'crates/rustok-pricing/src/lib.rs',
  owner: 'crates/rustok-pricing/src/ports.rs',
  wrapper: 'crates/rustok-pricing/src/read_context.rs',
  writeWrapper: 'crates/rustok-pricing/src/write_context.rs',
  evidence:
    'crates/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source.json',
  review:
    'crates/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source-review.json',
  document: 'crates/rustok-pricing/docs/read-local-context.md',
};

const lib = read(paths.lib);
const owner = read(paths.owner);
const wrapper = read(paths.wrapper);
const writeWrapper = read(paths.writeWrapper);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

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

for (const [source, value, label] of [
  [lib, 'mod read_context;', 'private wrapper module'],
  [
    lib,
    'pub use read_context::{InProcessPricingReadPort, in_process_pricing_read_port};',
    'canonical root wrapper export',
  ],
  [lib, 'in_process_pricing_write_port,', 'unchanged root write factory'],
  [owner, 'pub fn in_process_pricing_read_port(', 'legacy compatibility factory'],
  [owner, 'impl PricingReadPort for crate::PricingService', 'owner implementation'],
  [wrapper, 'pub struct InProcessPricingReadPort', 'canonical read wrapper'],
  [wrapper, 'pub fn from_service(inner: PricingService) -> Self', 'composition constructor'],
  [wrapper, 'pub fn in_process_pricing_read_port(', 'canonical read factory'],
  [wrapper, 'Arc::new(InProcessPricingReadPort::new(db, event_bus))', 'wrapper construction'],
  [wrapper, 'impl PricingReadPort for InProcessPricingReadPort', 'wrapper implementation'],
  [wrapper, 'const PRICING_OWNER: &str = "rustok_pricing";', 'truthful owner'],
  [wrapper, 'const PRICING_READ_BOUNDARY: &str = "pricing_read_port";', 'stable boundary'],
]) requireText(source, value, label);

forbidText(lib, 'pub use ports::*;', 'wildcard root compatibility export');
forbidText(
  lib,
  'pub use ports::in_process_pricing_read_port',
  'legacy read factory exported as canonical root',
);

const operations = [
  ['resolve_product_price', 'RESOLVE_PRODUCT_PRICE_OPERATION'],
  ['read_price_list_projection', 'READ_PRICE_LIST_PROJECTION_OPERATION'],
  ['list_active_price_list_projections', 'LIST_ACTIVE_PRICE_LIST_PROJECTIONS_OPERATION'],
  ['read_admin_product_pricing_projection', 'READ_ADMIN_PRODUCT_PRICING_PROJECTION_OPERATION'],
  ['read_storefront_product_pricing_projection', 'READ_STOREFRONT_PRODUCT_PRICING_PROJECTION_OPERATION'],
  ['preview_variant_discount', 'PREVIEW_VARIANT_DISCOUNT_OPERATION'],
];
for (const [operation, constant] of operations) {
  requireText(wrapper, `PricingReadPort::${operation}(`, `${operation} owner delegation`);
  requireText(wrapper, constant, `${operation} stable operation constant`);
}
const delegations = wrapper.match(/&self\.inner/g) ?? [];
if (delegations.length !== operations.length) {
  failures.push(`expected ${operations.length} owner delegations, found ${delegations.length}`);
}

for (const marker of [
  'struct PricingReadContextFacts',
  "actor_kind: &'static str",
  'tenant_id_length: context.tenant_id.chars().count()',
  'actor_id_length: context.actor.id.chars().count()',
  'claim_count: context.claims.len()',
  'role_count: context.roles.len()',
  'channel_present: context.channel.is_some()',
  'channel_length: context.channel.as_ref().map(',
  'locale_length: context.locale.chars().count()',
  'causation_id_present: context.causation_id.is_some()',
  'traceparent_present: context.traceparent.is_some()',
  'idempotency_key_present: context.idempotency_key.is_some()',
  'deadline_ms: context.deadline_ms',
]) requireText(wrapper, marker, 'bounded delegated context');

for (const marker of [
  'product_id_present: bool',
  'product_id_non_nil: bool',
  'variant_id_present: bool',
  'variant_id_non_nil: bool',
  'region_id_present: bool',
  'region_id_non_nil: bool',
  'channel_id_present: bool',
  'channel_id_non_nil: bool',
  'price_list_id_present: bool',
  'price_list_id_non_nil: bool',
  'selected_price_list_id_present: bool',
  'selected_price_list_id_non_nil: bool',
  'quantity_present: bool',
  'quantity_nonzero: bool',
  'quantity_negative: bool',
  'currency_code_length: Option<usize>',
  'channel_slug_length: Option<usize>',
  'locale_length: Option<usize>',
  'fallback_locale_length: Option<usize>',
  'handle_length: Option<usize>',
  'public_channel_slug_length: Option<usize>',
]) requireText(wrapper, marker, 'bounded request fact schema');

const logger = functionBody(wrapper, 'log_pricing_read_local_outcome');
for (const marker of [
  'tracing::error!(',
  'tracing::warn!(',
  'owner = PRICING_OWNER',
  'operation = owner_operation',
  'local_operation = outcome.local_operation',
  'correlation_id = %context.correlation_id',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'product_id_present = facts.product_id_present',
  'variant_id_non_nil = facts.variant_id_non_nil',
  'quantity_present = facts.quantity_present',
  'quantity_nonzero = facts.quantity_nonzero',
  'quantity_negative = facts.quantity_negative',
  'public_message_present',
  'public_message_length',
  'original_message_length',
  'error_kind',
  'retryable = mapped_error.retryable',
  'boundary = PRICING_READ_BOUNDARY',
]) requireText(logger, marker, 'bounded local outcome logger');

const kind = functionBody(wrapper, 'pricing_read_port_error_kind');
for (const marker of [
  'PortErrorKind::Validation => "validation"',
  'PortErrorKind::NotFound => "not_found"',
  'PortErrorKind::Conflict => "conflict"',
  'PortErrorKind::Forbidden => "forbidden"',
  'PortErrorKind::Unavailable => "unavailable"',
  'PortErrorKind::Timeout => "timeout"',
  'PortErrorKind::InvariantViolation => "invariant_violation"',
]) requireText(kind, marker, 'closed PortErrorKind label');

const sanitizedOutcomes = [
  ['pricing.tenant_id_invalid', 'pricing request context is invalid'],
  ['pricing.variant_product_mismatch', 'variant does not belong to the requested product'],
  ['pricing.price_not_found', 'price was not found'],
  ['pricing.price_list_not_found', 'price list was not found'],
  ['pricing.product_not_found', 'product was not found'],
  ['pricing.variant_not_found', 'variant was not found'],
  ['pricing.duplicate_handle', 'pricing handle is already in use'],
  ['pricing.duplicate_sku', 'pricing SKU is already in use'],
  ['pricing.insufficient_inventory', 'inventory is insufficient for the pricing operation'],
  ['pricing.shipping_profile_not_found', 'shipping profile was not found'],
  ['pricing.duplicate_shipping_profile_slug', 'shipping profile slug is already in use'],
];
for (const [code, message] of sanitizedOutcomes) {
  requireText(wrapper, `"${code}"`, `${code} classification`);
  requireText(wrapper, `Some("${message}")`, `${code} stable message`);
}

const mapper = functionBody(wrapper, 'map_pricing_read_local_port_error');
for (const marker of [
  'return error;',
  'PortError::new(',
  'error.kind.clone()',
  'error.code.clone()',
  'error.retryable',
  'None => error.clone()',
  'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
  'log_pricing_read_local_outcome(',
  'mapped_error',
]) requireText(mapper, marker, 'same envelope and severity mapping');

for (const forbidden of [
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
  'product_id = ?facts.product_id',
  'variant_id = ?facts.variant_id',
  'region_id = ?facts.region_id',
  'channel_id = ?facts.channel_id',
  'price_list_id = ?facts.price_list_id',
  'selected_price_list_id = ?facts.selected_price_list_id',
  'quantity = ?facts.quantity',
  'public_message = %mapped_error.message',
  'error_kind = ?mapped_error.kind',
  'error = ?error',
  'internal_message = %error.message',
  'original_message =',
  'handle = %',
  'handle = ?',
  'channel_slug = %',
  'channel_slug = ?',
  'currency_code = %',
  'currency_code = ?',
  'discount_percent =',
  'amount =',
  'compare_at_amount =',
]) forbidText(wrapper, forbidden, 'raw pricing read diagnostics');

for (const marker of [
  'const PRICING_WRITE_BOUNDARY: &str = "pricing_write_port";',
  'impl PricingWritePort for InProcessPricingWritePort',
]) requireText(writeWrapper, marker, 'write wrapper remains separate');

for (const [key, expected] of Object.entries({
  read_operation_count: 6,
  owner_delegation_changed: false,
  request_response_contract_changed: false,
  error_kind_code_retryability_changed: false,
  public_message_mapping_changed: false,
  technical_error_severity_changed: false,
  ordinary_warning_severity_changed: false,
  raw_context_logged: false,
  raw_uuid_logged: false,
  exact_quantity_logged: false,
  public_message_logged: false,
  error_kind_debug_logged: false,
  bounded_context_shape_logged: true,
  bounded_request_shape_logged: true,
  message_shape_logged: true,
  closed_error_kind_logged: true,
  read_context_wrapper_cleanup_closed: true,
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
  all_six_read_operations_preserved: true,
  owner_delegation_preserved: true,
  public_error_envelope_preserved: true,
  public_message_mapping_preserved: true,
  technical_error_severity_preserved: true,
  ordinary_warning_severity_preserved: true,
  raw_context_removed: true,
  raw_uuid_removed: true,
  exact_quantity_removed: true,
  public_message_text_removed: true,
  debug_error_kind_removed: true,
  bounded_context_shape_retained: true,
  bounded_request_shape_retained: true,
  message_shape_retained: true,
  closed_error_kind_retained: true,
  read_wrapper_source_closed: true,
  write_wrapper_remains_open: true,
  broad_ecommerce_cleanup_remains_open: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  '# Pricing read local outcome context',
  'Status: **source-ready / unvalidated**',
  'bounded delegated context',
  'UUID presence and non-nil state',
  'exact quantity values are not recorded',
  'public message text is not recorded',
  '`crates/rustok-pricing/src/write_context.rs` remains open',
]) requireText(document, marker, `${paths.document}: truthful scope`);

if (failures.length > 0) {
  console.error('Pricing read local diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ canonical Pricing reads preserve six owner delegations and public envelopes while logging only bounded context/request/message shape; write and runtime evidence remain open',
);
