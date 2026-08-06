#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/mod.rs');
const shipping = read('crates/rustok-commerce/src/storefront_shipping.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
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

const mapper = between(
  controller,
  'fn map_storefront_cart_shipping_error(',
  'pub fn axum_router()',
  'cart shipping HTTP mapper',
);
const enrichmentHandler = between(
  controller,
  'pub(crate) async fn enrich_storefront_cart_for_db(',
  'pub(crate) fn requested_cart_context(',
  'cart enrichment HTTP helper',
);
const selectionHandler = between(
  controller,
  'pub(crate) async fn validate_selected_shipping_option_for_db(',
  'pub(crate) fn current_shipping_selections(',
  'selected shipping option helper',
);
const typedEnrichment = between(
  shipping,
  'pub async fn enrich_cart_delivery_groups_typed(',
  'pub async fn enrich_cart_delivery_groups(',
  'typed shipping enrichment',
);
const compatibilityWrapper = between(
  shipping,
  'pub async fn enrich_cart_delivery_groups(',
  'fn extract_allowed_shipping_profile_slugs_from_metadata(',
  'commerce compatibility wrapper',
);

for (const [value, label] of [
  ['FulfillmentService, error::FulfillmentError', 'typed fulfillment error import'],
  ['enrich_cart_delivery_groups_typed', 'typed enrichment import'],
  ['fn map_storefront_cart_shipping_error(', 'cart shipping mapper'],
  ['boundary = "commerce_storefront_cart_shipping_http"', 'cart shipping boundary'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment transition variant'],
  ['FulfillmentError::Database(_)', 'fulfillment database variant'],
  ['StatusCode::BAD_REQUEST', 'shipping invalid status'],
  ['StatusCode::NOT_FOUND', 'shipping not-found status'],
  ['StatusCode::CONFLICT', 'shipping conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'shipping unavailable status'],
  ['"commerce_store_shipping_invalid"', 'shipping invalid code'],
  ['"commerce_store_not_found"', 'shipping not-found code'],
  ['"commerce_store_shipping_state_conflict"', 'shipping conflict code'],
  ['"commerce_store_shipping_unavailable"', 'shipping unavailable code'],
  ['"Shipping request is invalid"', 'static invalid message'],
  ['"Commerce resource not found"', 'static not-found message'],
  ['"Shipping operation conflicts with the current state"', 'static conflict message'],
  ['"Shipping service is temporarily unavailable"', 'static unavailable message'],
  ['error = ?error', 'raw internal error logging'],
  ['owner = "rustok_fulfillment"', 'fulfillment owner logging'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['cart_id = %cart_id', 'cart logging'],
  ['error_kind,', 'typed error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['HttpError::new(status, code, message)', 'static public envelope'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['let cart_id = cart.id;', 'cart identity capture'],
  ['enrich_cart_delivery_groups_typed(', 'typed enrichment call'],
  ['public_channel_slug.as_deref()', 'public channel propagation'],
  ['Some(request_context.locale.as_str())', 'requested locale propagation'],
  ['Some(tenant_default_locale)', 'tenant locale propagation'],
  ['"enrich_cart_delivery_groups"', 'enrichment operation label'],
  ['map_storefront_cart_shipping_error(', 'typed enrichment mapper use'],
  ['if cart.delivery_groups.len() == 1', 'single-group reconciliation'],
  ['available_shipping_options', 'available option projection'],
  ['cart.selected_shipping_option_id = selected_id', 'selected option reconciliation'],
]) {
  requireText(enrichmentHandler, value, label);
}
for (const value of [
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("commerce_store_invalid"',
]) {
  forbidText(enrichmentHandler, value, 'unsafe enrichment public conversion');
}

for (const [value, label] of [
  ['FulfillmentService::new(db.clone())', 'fulfillment service construction'],
  ['if cart.delivery_groups.len() > 1', 'multi-group legacy selection guard'],
  ['current_shipping_selections(cart)', 'current selection fallback'],
  ['normalize_shipping_profile_slug(', 'profile normalization'],
  ['.get_shipping_option(', 'typed shipping option read'],
  ['selected_shipping_option_id,', 'selected option identity'],
  ['validation.requested_locale', 'requested locale propagation'],
  ['validation.tenant_default_locale', 'tenant locale propagation'],
  ['"get_selected_shipping_option"', 'selection operation label'],
  ['map_storefront_cart_shipping_error(', 'typed option mapper use'],
  ['eq_ignore_ascii_case(validation.currency_code)', 'currency compatibility check'],
  ['is_metadata_visible_for_public_channel', 'channel visibility check'],
  ['is_shipping_option_compatible_with_profiles', 'shipping profile compatibility check'],
]) {
  requireText(selectionHandler, value, label);
}
for (const value of [
  '.map_err(|err| HttpError::bad_request("commerce_store_invalid", err.to_string()))',
  '.map_err(|error| HttpError::bad_request("commerce_store_invalid", error.to_string()))',
]) {
  forbidText(selectionHandler, value, 'unsafe option lookup public conversion');
}

for (const [value, label] of [
  ['use rustok_fulfillment::{FulfillmentResult, FulfillmentService};', 'typed fulfillment result import'],
  ['-> FulfillmentResult<CartResponse>', 'typed enrichment return'],
  ['FulfillmentService::new(db.clone())', 'typed fulfillment service'],
  ['.list_shipping_options(tenant_id, requested_locale, tenant_default_locale)', 'typed option list'],
  ['.await?;', 'direct typed error propagation'],
  ['eq_ignore_ascii_case(&cart.currency_code)', 'currency filtering'],
  ['is_metadata_visible_for_public_channel(', 'channel filtering'],
  ['is_shipping_option_compatible_with_profiles(option, &required_profiles)', 'profile filtering'],
  ['map_shipping_option_summary', 'option summary projection'],
  ['delivery_group.selected_shipping_option_id = Some(selected_id)', 'legacy selected option propagation'],
  ['cart.delivery_groups[0].selected_shipping_option_id', 'single-group cart projection'],
]) {
  requireText(typedEnrichment, value, label);
}
for (const value of [
  'CommerceError::Validation',
  'error.to_string()',
  'err.to_string()',
]) {
  forbidText(typedEnrichment, value, 'typed enrichment error erasure');
}

for (const [value, label] of [
  ['-> CommerceResult<CartResponse>', 'legacy commerce return contract'],
  ['enrich_cart_delivery_groups_typed(', 'typed implementation delegation'],
  [
    'crate::CommerceError::Validation(\n            "Cart shipping details are temporarily unavailable".to_string(),\n        )',
    'stable GraphQL compatibility mapping',
  ],
]) {
  requireText(compatibilityWrapper, value, label);
}
for (const value of ['error.to_string()', 'err.to_string()', 'format!("{error:?}")']) {
  forbidText(compatibilityWrapper, value, 'unsafe legacy GraphQL compatibility mapping');
}

for (const [content, value, label] of [
  [fulfillmentErrors, 'pub enum FulfillmentError {', 'owner fulfillment enum'],
  [fulfillmentErrors, 'Validation(String)', 'owner validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner database variant'],
]) {
  requireText(content, value, label);
}

const mapperUses = controller.match(/map_storefront_cart_shipping_error\(/g) ?? [];
if (mapperUses.length !== 3) {
  failures.push(`expected mapper definition plus two HTTP uses, found ${mapperUses.length}`);
}
const typedEnrichmentDefinitions = shipping.match(/pub async fn enrich_cart_delivery_groups_typed\(/g) ?? [];
if (typedEnrichmentDefinitions.length !== 1) {
  failures.push(`expected one typed enrichment definition, found ${typedEnrichmentDefinitions.length}`);
}
const compatibilityDefinitions = shipping.match(/pub async fn enrich_cart_delivery_groups\(/g) ?? [];
if (compatibilityDefinitions.length !== 1) {
  failures.push(`expected one commerce compatibility wrapper, found ${compatibilityDefinitions.length}`);
}

for (const [value, label] of [
  ['pub fn axum_router()', 'storefront router'],
  ['pub(crate) async fn apply_cart_context_patch_for_db(', 'cart context patch helper'],
  ['pub(crate) async fn reprice_storefront_cart_line_items_for_db(', 'cart repricing helper'],
  ['pub(crate) fn storefront_cart_pricing_snapshot(', 'pricing snapshot helper'],
  ['pub(crate) fn requested_cart_context(', 'requested context helper'],
  ['pub(crate) fn current_shipping_selections(', 'current selections helper'],
  ['pub(crate) async fn resolve_store_line_item_input(', 'line-item resolution helper'],
  ['pub(crate) async fn validate_store_variant_inventory(', 'inventory validation helper'],
  ['pub struct StoreCreateCartInput', 'create cart DTO'],
  ['pub struct StoreCompleteCartInput', 'checkout DTO'],
  ['pub struct StoreCartShippingSelectionInput', 'shipping selection DTO'],
]) {
  requireText(controller, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront cart shipping HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Storefront cart shipping helpers preserve typed safe fulfillment envelopes');
