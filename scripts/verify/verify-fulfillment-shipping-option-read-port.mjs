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

const ownerRoot = read('crates/rustok-fulfillment/src/lib.rs');
const ownerSource = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const errorSource = read('crates/rustok-fulfillment/src/error.rs');
const contextSource = read(
  'crates/rustok-commerce/src/graphql/mutations/shipping_option_read_context.rs',
);
const optionSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs',
);
const enrichmentSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_enrichment_helper.rs',
);
const projectionSource = read('crates/rustok-commerce/src/storefront_shipping.rs');
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/shipping-option-read-diagnostic-safety-source.json',
  ),
);
const review = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/shipping-option-read-diagnostic-safety-source-review.json',
  ),
);
const diagnosticDoc = read(
  'crates/rustok-fulfillment/docs/shipping-option-read-diagnostic-safety.md',
);
const plan = read('crates/rustok-fulfillment/docs/implementation-plan.md');

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [ownerRoot, 'mod shipping_option_read;', 'private owner module'],
  [ownerRoot, 'ShippingOptionReadPort,', 'storefront trait export'],
  [ownerRoot, 'ShippingOptionAdminReadPort,', 'admin trait export'],
  [ownerRoot, 'in_process_shipping_option_read_port,', 'storefront factory export'],
  [ownerRoot, 'in_process_shipping_option_admin_read_port,', 'admin factory export'],
  [ownerSource, 'pub trait ShippingOptionReadPort: Send + Sync {', 'storefront read trait'],
  [ownerSource, 'pub trait ShippingOptionAdminReadPort: Send + Sync {', 'admin read trait'],
  [ownerSource, 'impl ShippingOptionReadPort for InProcessShippingOptionReadPort', 'storefront implementation'],
  [ownerSource, 'impl ShippingOptionAdminReadPort for InProcessShippingOptionAdminReadPort', 'admin implementation'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['context.require_policy(PortCallPolicy::read())?', 'read admission'],
  ['parse_tenant_id(&context, "list_shipping_option_projections")?', 'active-list tenant parse'],
  ['parse_tenant_id(&context, "read_shipping_option_projection")?', 'lookup tenant parse'],
  ['parse_tenant_id(&context, "list_all_shipping_option_projections")?', 'list-all tenant parse'],
  ['.list_shipping_options(', 'active-list owner delegation'],
  ['.get_shipping_option(', 'lookup owner delegation'],
  ['.list_all_shipping_options(', 'list-all owner delegation'],
  ['request.requested_locale.as_deref()', 'requested locale delegation'],
  ['request.tenant_default_locale.as_deref()', 'default locale delegation'],
]) requireText(ownerSource, value, label);

for (const [value, label] of [
  ['ShippingOptionReadContextFacts', 'bounded context facts'],
  ['ShippingOptionOwnerErrorFacts', 'bounded owner-error facts'],
  ['ShippingOptionReadRequestFacts', 'bounded request facts'],
  ['tenant_id_parse_failed = true', 'tenant parse failure fact'],
  ['shipping_option_id_present = request_facts.shipping_option_id_present', 'identity presence fact'],
  ['shipping_option_id_non_nil = request_facts.shipping_option_id_non_nil', 'identity shape fact'],
  ['requested_locale_present = request_facts.requested_locale_present', 'requested locale presence'],
  ['tenant_default_locale_present = request_facts.tenant_default_locale_present', 'default locale presence'],
  ['error_variant = error_facts.error_variant', 'static error variant'],
  ['text_total_length = error_facts.text_total_length', 'text shape'],
  ['uuid_non_nil_count = error_facts.uuid_non_nil_count', 'uuid shape'],
  ['opaque_payload_present = error_facts.opaque_payload_present', 'opaque payload presence'],
  ['boundary = SHIPPING_OPTION_READ_BOUNDARY', 'owner boundary'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
]) requireText(ownerSource, value, label);

for (const value of [
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'shipping_option_id = ?shipping_option_id',
  'error = %message',
  'resource_id = %',
  'from = %from',
  'to = %to',
  'requested_locale = %',
  'requested_locale = ?',
  'tenant_default_locale = %',
  'tenant_default_locale = ?',
]) forbidText(ownerSource, value, 'shipping-option owner diagnostics');

for (const marker of [
  'Validation(String)',
  'ShippingOptionNotFound(Uuid)',
  'FulfillmentNotFound(Uuid)',
  'InvalidTransition { from: String, to: String }',
  'Database(#[from] DbErr)',
]) requireText(errorSource, marker, 'retained owner error shape');

for (const [source, value, label] of [
  [contextSource, 'PortActor::service("rustok-commerce.storefront-shipping")', 'commerce service actor'],
  [contextSource, 'format!("storefront-shipping:{operation}:{cart_id}")', 'commerce correlation'],
  [contextSource, '.with_deadline(std::time::Duration::from_secs(2))', 'commerce deadline'],
  [contextSource, 'context.clone().with_channel(channel)', 'commerce channel propagation'],
  [contextSource, 'rustok_fulfillment::in_process_shipping_option_read_port(db)', 'commerce root factory'],
  [optionSource, '.read_shipping_option_projection(', 'mounted lookup'],
  [enrichmentSource, '.list_shipping_option_projections(', 'mounted active-list'],
  [projectionSource, 'pub fn enrich_cart_delivery_groups_from_options(', 'pure projection'],
]) requireText(source, value, label);

for (const source of [optionSource, enrichmentSource]) {
  for (const value of [
    'FulfillmentService::new(',
    '.get_shipping_option(',
    '.list_shipping_options(',
    'FulfillmentError',
    'error.message',
  ]) forbidText(source, value, 'mounted shipping-option topology');
}

for (const [key, expected] of Object.entries({
  complete_fulfillment_error_logged: false,
  database_error_payload_logged: false,
  uuid_parser_payload_logged: false,
  resource_uuid_logged: false,
  raw_context_values_logged: false,
  static_error_variant_logged: true,
  aggregate_error_shape_logged: true,
  safe_context_shape_logged: true,
  request_identity_shape_logged: true,
  locale_shape_logged: true,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  fulfillment_lifecycle_read_diagnostic_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`diagnostic evidence ${key} must be ${expected}`);
  }
}
if (evidence.validation?.compile_proven !== false) {
  failures.push('diagnostic evidence must not claim compilation');
}
for (const [key, expected] of Object.entries({
  public_traits_preserved: true,
  read_admission_order_preserved: true,
  owner_delegation_preserved: true,
  locale_delegation_preserved: true,
  all_public_port_errors_preserved: true,
  complete_fulfillment_error_logging_removed: true,
  raw_context_values_removed: true,
  resource_uuid_removed: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`source review ${key} must be ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'Fulfillment lifecycle projection-read diagnostics',
]) requireText(diagnosticDoc, marker, 'diagnostic documentation');
for (const marker of [
  'Shipping-option projection-read owner payload diagnostics are source-closed / unvalidated.',
  'verify-fulfillment-shipping-option-read-port.mjs',
]) requireText(plan, marker, 'implementation plan');

if (failures.length > 0) {
  console.error('Fulfillment shipping-option read port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment shipping-option projection reads retain owner topology, bounded diagnostics, and stable public PortError behavior',
);
