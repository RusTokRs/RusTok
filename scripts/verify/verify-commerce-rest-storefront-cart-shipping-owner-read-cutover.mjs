#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const carts = read('crates/rustok-commerce/src/controllers/store/carts.rs');
const helper = read(
  'crates/rustok-commerce/src/controllers/store/carts/shipping_owner_reads.rs',
);
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const shippingOwner = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/rest-storefront-cart-shipping-owner-read-cutover-2026-08-09.md',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['mod shipping_owner_reads;', 'mounted helper module'],
  ['runtime.shipping_option_read_port()', 'host-selected shipping port access'],
  ['shipping_owner_reads::enrich_storefront_cart(', 'mounted enrichment handoff'],
  ['shipping_owner_reads::apply_cart_context_patch(', 'mounted context validation handoff'],
]) requireText(carts, value, label);

const runtimePortUses = carts.match(/runtime\.shipping_option_read_port\(\)/g) ?? [];
if (runtimePortUses.length !== 6) {
  failures.push(`expected six mounted shipping read-port acquisitions, found ${runtimePortUses.length}`);
}

for (const value of [
  'super::enrich_storefront_cart_for_db(',
  'super::apply_cart_context_patch_for_db(',
  'FulfillmentService::new(',
  'rustok_fulfillment::FulfillmentService',
]) forbidText(carts, value, 'stale mounted Fulfillment construction');

for (const [value, label] of [
  ['fn shipping_read_context(', 'shipping read context builder'],
  ['super::super::storefront_cart_port_context(', 'trusted storefront context reuse'],
  ['false,', 'read policy context'],
  ['ShippingOptionReadPort', 'owner read trait'],
  ['ListShippingOptionProjectionsRequest', 'owner list request'],
  ['.list_shipping_option_projections(', 'owner list call'],
  ['ReadShippingOptionProjectionRequest', 'owner detail request'],
  ['.read_shipping_option_projection(', 'owner detail call'],
  ['requested_locale: Some(request_context.locale.clone())', 'requested locale forwarding'],
  ['tenant_default_locale: Some(tenant_default_locale.to_string())', 'tenant locale forwarding'],
  ['enrich_cart_delivery_groups_from_options(', 'existing enrichment projection reuse'],
  ['normalize_shipping_profile_slug(', 'profile normalization'],
  ['is_metadata_visible_for_public_channel(', 'public channel validation'],
  ['is_shipping_option_compatible_with_profiles(', 'profile compatibility validation'],
  ['eq_ignore_ascii_case(validation.currency_code)', 'currency compatibility validation'],
  ['super::super::requested_cart_context(', 'existing requested cart context'],
  ['super::super::resolve_context_for_db(', 'StoreContext resolution order'],
  ['.update_storefront_context(', 'Cart owner context update'],
  ['super::super::reprice_storefront_cart_line_items_for_db(', 'repricing order'],
  ['enrich_storefront_cart(', 'post-reprice enrichment'],
]) requireText(helper, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation mapping'],
  ['PortErrorKind::NotFound', 'not-found mapping'],
  ['PortErrorKind::Conflict', 'conflict mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable mapping'],
  ['PortErrorKind::Forbidden | PortErrorKind::InvariantViolation', 'fail-closed mapping'],
  ['"commerce_store_shipping_invalid"', 'validation public code'],
  ['"commerce_store_not_found"', 'not-found public code'],
  ['"commerce_store_shipping_state_conflict"', 'conflict public code'],
  ['"commerce_store_shipping_unavailable"', 'unavailable public code'],
  ['"commerce_store_shipping_failed"', 'unexpected owner failure public code'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostics'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['retryable = error.retryable', 'retryability diagnostic'],
  ['HttpError::new(status, code, message)', 'stable public envelope'],
]) requireText(helper, value, label);

for (const value of [
  'FulfillmentService::new(',
  'FulfillmentError',
  'error = ?error',
  'error.message',
  'error.to_string()',
  'err.to_string()',
]) forbidText(helper, value, 'raw or concrete Fulfillment dependency');

for (const [value, label] of [
  ['shipping_option_read_runtime: crate::graphql_runtime::CommerceShippingOptionReadRuntime', 'HTTP runtime shipping capability'],
  ['fn shipping_option_read_port(', 'HTTP runtime read-port accessor'],
  ['self.shipping_option_read_runtime\n            .shipping_option_read_port()', 'HTTP runtime trait projection'],
  ['shared_get::<crate::graphql_runtime::CommerceShippingOptionReadRuntime>()', 'host-selected HTTP runtime'],
]) requireText(httpRuntime, value, label);

for (const [value, label] of [
  ['pub trait ShippingOptionReadPort', 'Fulfillment owner read port'],
  ['async fn list_shipping_option_projections(', 'Fulfillment owner list operation'],
  ['async fn read_shipping_option_projection(', 'Fulfillment owner detail operation'],
  ['context.require_policy(PortCallPolicy::read())?', 'Fulfillment read admission'],
]) requireText(shippingOwner, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology item remains open',
);

for (const [value, label] of [
  ['# Commerce REST storefront cart shipping owner-read cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record source status'],
  ['`ShippingOptionReadPort::list_shipping_option_projections`', 'record owner list'],
  ['`ShippingOptionReadPort::read_shipping_option_projection`', 'record owner detail'],
  ['The canonical ecommerce topology item remains open', 'record broad P0 open'],
  ['No tests, Cargo commands, Node verifiers, formatter', 'record no validation execution'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce REST storefront cart shipping owner-read cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ mounted REST cart shipping validation and enrichment use host-selected Fulfillment owner reads',
);
