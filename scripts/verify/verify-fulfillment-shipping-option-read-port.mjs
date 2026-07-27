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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [source, value, label] of [
  [ownerRoot, 'mod shipping_option_read;', 'private owner module'],
  [ownerRoot, 'InProcessShippingOptionReadPort,', 'wrapper export'],
  [ownerRoot, 'ShippingOptionReadPort,', 'trait export'],
  [ownerRoot, 'in_process_shipping_option_read_port,', 'root factory export'],
  [ownerSource, 'pub trait ShippingOptionReadPort: Send + Sync {', 'read port trait'],
  [ownerSource, 'pub struct InProcessShippingOptionReadPort {', 'in-process wrapper'],
  [ownerSource, 'impl ShippingOptionReadPort for InProcessShippingOptionReadPort', 'wrapper implementation'],
  [
    ownerSource,
    'pub fn in_process_shipping_option_read_port(',
    'canonical factory',
  ],
]) {
  requireText(source, value, label);
}

for (const [value, label] of [
  ['async fn list_shipping_option_projections(', 'list operation'],
  ['async fn read_shipping_option_projection(', 'read operation'],
  ['ListShippingOptionProjectionsRequest', 'list request'],
  ['ReadShippingOptionProjectionRequest', 'read request'],
  ['pub requested_locale: Option<String>', 'requested locale'],
  ['pub tenant_default_locale: Option<String>', 'default locale'],
  ['pub shipping_option_id: Uuid', 'option identity'],
  ['context.require_policy(PortCallPolicy::read())?', 'read admission policy'],
  ['parse_tenant_id(&context, "list_shipping_option_projections")?', 'list tenant parse'],
  ['parse_tenant_id(&context, "read_shipping_option_projection")?', 'read tenant parse'],
  ['.list_shipping_options(', 'owner list delegation'],
  ['.get_shipping_option(', 'owner read delegation'],
  ['request.requested_locale.as_deref()', 'requested locale delegation'],
  ['request.tenant_default_locale.as_deref()', 'default locale delegation'],
]) {
  requireText(ownerSource, value, label);
}

const listDelegations = ownerSource.match(/\.list_shipping_options\(/g) ?? [];
if (listDelegations.length !== 1) {
  failures.push(`expected one owner list delegation, found ${listDelegations.length}`);
}
const readDelegations = ownerSource.match(/\.get_shipping_option\(/g) ?? [];
if (readDelegations.length !== 1) {
  failures.push(`expected one owner option delegation, found ${readDelegations.length}`);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'conflict mapping'],
  ['FulfillmentError::Database(_)', 'database mapping'],
  ['PortErrorKind::Validation', 'validation kind'],
  ['PortErrorKind::NotFound', 'not-found kind'],
  ['PortErrorKind::Conflict', 'conflict kind'],
  ['PortErrorKind::Unavailable', 'unavailable kind'],
  ['"fulfillment.validation"', 'validation code'],
  ['"fulfillment.shipping_option_not_found"', 'shipping option code'],
  ['"fulfillment.fulfillment_not_found"', 'fulfillment code'],
  ['"fulfillment.invalid_transition"', 'conflict code'],
  ['"fulfillment.database_unavailable"', 'database code'],
  ['PortError::new(kind, code, message, retryable)', 'stable error construction'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['correlation_id = %context.correlation_id', 'correlation fact'],
  ['tenant_id = %context.tenant_id', 'tenant fact'],
  ['actor = ?context.actor', 'actor fact'],
  ['channel_length = context.channel.as_deref().map(str::len)', 'channel length'],
  ['locale_length = context.locale.len()', 'locale length'],
  ['causation_id_present = context.causation_id.is_some()', 'causation presence'],
  ['traceparent_present = context.traceparent.is_some()', 'trace presence'],
  ['deadline_ms = ?context.deadline_ms', 'deadline fact'],
  ['shipping_option_id = ?shipping_option_id', 'option identity fact'],
  ['requested_locale_length = ?requested_locale_length', 'requested locale length'],
  ['tenant_default_locale_length = ?tenant_default_locale_length', 'default locale length'],
  ['boundary = "fulfillment_shipping_option_read_port"', 'owner boundary'],
  ['tracing::error!(', 'technical severity'],
  ['tracing::warn!(', 'ordinary severity'],
]) {
  requireText(ownerSource, value, label);
}

for (const [value, label] of [
  ['PortActor::service("rustok-commerce.storefront-shipping")', 'commerce service actor'],
  ['format!("storefront-shipping:{operation}:{cart_id}")', 'commerce correlation'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'commerce deadline'],
  ['context.clone().with_channel(channel)', 'commerce channel propagation'],
  [
    'rustok_fulfillment::in_process_shipping_option_read_port(db)',
    'commerce root factory usage',
  ],
]) {
  requireText(contextSource, value, label);
}

for (const [source, value, label] of [
  [optionSource, '.read_shipping_option_projection(', 'mounted option read'],
  [optionSource, 'ReadShippingOptionProjectionRequest {', 'mounted option request'],
  [enrichmentSource, '.list_shipping_option_projections(', 'mounted option list'],
  [enrichmentSource, 'ListShippingOptionProjectionsRequest {', 'mounted list request'],
  [
    projectionSource,
    'pub fn enrich_cart_delivery_groups_from_options(',
    'pure commerce projection',
  ],
  [
    enrichmentSource,
    'enrich_cart_delivery_groups_from_options(',
    'mounted pure projection usage',
  ],
]) {
  requireText(source, value, label);
}

for (const source of [optionSource, enrichmentSource]) {
  for (const value of [
    'FulfillmentService::new(',
    '.get_shipping_option(',
    '.list_shipping_options(',
    'FulfillmentError',
    'error.message',
  ]) {
    forbidText(source, value, 'mounted shipping-option read topology');
  }
}

for (const value of [
  'error = %message',
  'message = %',
  'requested_locale = %',
  'requested_locale = ?',
  'tenant_default_locale = %',
  'tenant_default_locale = ?',
]) {
  forbidText(ownerSource, value, 'shipping-option owner diagnostics');
}

if (failures.length > 0) {
  console.error('Fulfillment shipping-option read port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment owns complete shipping-option reads behind a canonical read port and mounted commerce uses retained read context without concrete service construction',
);
