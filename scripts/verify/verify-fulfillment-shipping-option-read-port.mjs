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
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const ownerRoot = read('crates/rustok-fulfillment/src/lib.rs');
const owner = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const context = read(
  'crates/rustok-commerce/src/graphql/mutations/shipping_option_read_context.rs',
);
const optionConsumer = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_option_helper.rs',
);
const listConsumer = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_shipping_enrichment_helper.rs',
);
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
const doc = read(
  'crates/rustok-fulfillment/docs/shipping-option-read-diagnostic-safety.md',
);

for (const marker of [
  'mod shipping_option_read;',
  'ShippingOptionReadPort,',
  'ShippingOptionAdminReadPort,',
  'in_process_shipping_option_read_port,',
  'in_process_shipping_option_admin_read_port,',
]) requireText(ownerRoot, marker, 'owner export');

for (const marker of [
  'pub trait ShippingOptionReadPort: Send + Sync {',
  'pub trait ShippingOptionAdminReadPort: Send + Sync {',
  'impl ShippingOptionReadPort for InProcessShippingOptionReadPort',
  'impl ShippingOptionAdminReadPort for InProcessShippingOptionAdminReadPort',
  'context.require_policy(PortCallPolicy::read())?',
  'parse_tenant_id(&context, "list_shipping_option_projections")?',
  'parse_tenant_id(&context, "read_shipping_option_projection")?',
  'parse_tenant_id(&context, "list_all_shipping_option_projections")?',
  '.list_shipping_options(',
  '.get_shipping_option(',
  '.list_all_shipping_options(',
  'request.requested_locale.as_deref()',
  'request.tenant_default_locale.as_deref()',
]) requireText(owner, marker, 'owner topology');

for (const marker of [
  'ShippingOptionReadContextFacts',
  'ShippingOptionOwnerErrorFacts',
  'ShippingOptionReadRequestFacts',
  'tenant_id_parse_failed = true',
  'shipping_option_id_present = request_facts.shipping_option_id_present',
  'shipping_option_id_non_nil = request_facts.shipping_option_id_non_nil',
  'requested_locale_present = request_facts.requested_locale_present',
  'tenant_default_locale_present = request_facts.tenant_default_locale_present',
  'error_variant = error_facts.error_variant',
  'text_total_length = error_facts.text_total_length',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'boundary = SHIPPING_OPTION_READ_BOUNDARY',
  'PortError::new(kind, code, message, retryable)',
  'tracing::error!(',
  'tracing::warn!(',
]) requireText(owner, marker, 'bounded diagnostics');

for (const marker of [
  '"fulfillment.context_invalid"',
  '"fulfillment.validation"',
  '"fulfillment.shipping_option_not_found"',
  '"fulfillment.fulfillment_not_found"',
  '"fulfillment.invalid_transition"',
  '"fulfillment.database_unavailable"',
  '"fulfillment request context is invalid"',
  '"fulfillment request is invalid"',
  '"shipping option was not found"',
  '"fulfillment was not found"',
  '"fulfillment lifecycle transition conflicts with the current state"',
  '"fulfillment storage is temporarily unavailable"',
]) requireText(owner, marker, 'stable public error');

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
]) forbidText(owner, value, 'complete owner payload');

for (const [source, marker, label] of [
  [context, 'PortActor::service("rustok-commerce.storefront-shipping")', 'commerce actor'],
  [context, '.with_deadline(std::time::Duration::from_secs(2))', 'commerce deadline'],
  [context, 'rustok_fulfillment::in_process_shipping_option_read_port(db)', 'root factory'],
  [optionConsumer, '.read_shipping_option_projection(', 'mounted lookup'],
  [listConsumer, '.list_shipping_option_projections(', 'mounted list'],
]) requireText(source, marker, label);
for (const source of [optionConsumer, listConsumer]) {
  for (const value of ['FulfillmentService::new(', 'FulfillmentError', 'error.message']) {
    forbidText(source, value, 'mounted consumer boundary');
  }
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
requireText(doc, 'Status: **source-ready / unvalidated**', 'diagnostic document');
requireText(doc, 'Fulfillment lifecycle projection-read diagnostics', 'remaining gap');

if (failures.length > 0) {
  console.error('Fulfillment shipping-option read port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log(
  '✔ fulfillment shipping-option projection reads retain owner topology, bounded diagnostics, and stable public PortError behavior',
);
