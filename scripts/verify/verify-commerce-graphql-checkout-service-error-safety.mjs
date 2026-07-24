#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const routing = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_checkout.rs');
const source = read('crates/rustok-commerce/src/graphql/mutations/checkout.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(
  routing,
  '#[path = "safe_checkout.rs"]\npub mod checkout;',
  'checkout safe module routing',
);
const checkoutModuleDeclarations = routing.match(/pub mod checkout;/g) ?? [];
if (checkoutModuleDeclarations.length !== 1) {
  failures.push(`expected one checkout module declaration, found ${checkoutModuleDeclarations.length}`);
}

for (const [value, label] of [
  ['mod checkout_boundary {', 'checkout boundary module'],
  ['#[derive(Clone)]', 'async GraphQL clone requirement'],
  ['pub(crate) enum BoundaryError {', 'local boundary error'],
  ['Graphql(Error)', 'GraphQL pass-through variant'],
  ['Public {', 'cloneable public envelope variant'],
  ['impl From<Error> for BoundaryError', 'GraphQL conversion'],
  ['impl From<CommerceError> for BoundaryError', 'commerce conversion'],
  ['impl From<FulfillmentError> for BoundaryError', 'fulfillment conversion'],
  ['Self::Public {', 'owner error envelope storage'],
  ['impl From<BoundaryError> for Error', 'public GraphQL conversion'],
  ['BoundaryError::Graphql(error) => error', 'existing GraphQL error preservation'],
  ['BoundaryError::Public {', 'public envelope restoration'],
  ['fn commerce_error_envelope(', 'shipping profile mapper'],
  ['fn fulfillment_error_envelope(', 'shipping option mapper'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['owner = "rustok_commerce"', 'commerce owner logging'],
  ['owner = "rustok_fulfillment"', 'fulfillment owner logging'],
  ['error_kind,', 'owner error kind logging'],
  ['public_code = code', 'public code logging'],
  ['boundary = "commerce_graphql_checkout"', 'checkout boundary logging'],
  ['mod async_graphql_shim {', 'async GraphQL result shim'],
  ['pub use ::async_graphql::{Context, Error, ErrorExtensions, Object};', 'GraphQL API re-export'],
  [
    'pub type Result<T> = std::result::Result<T, super::checkout_boundary::BoundaryError>;',
    'custom checkout result',
  ],
  ['use self::async_graphql_shim as async_graphql;', 'shim alias'],
  ['include!("checkout.rs");', 'unchanged checkout resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const value of [
  'Commerce(CommerceError)',
  'Fulfillment(FulfillmentError)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(facade, value, 'checkout facade public boundary');
}
for (const value of [
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(source, value, 'checkout resolver public boundary');
}

for (const [value, label] of [
  ['CommerceError::Validation(_)', 'commerce validation mapping'],
  ['CommerceError::InvalidPrice(_)', 'commerce invalid-price mapping'],
  ['CommerceError::InvalidOptionCombination', 'commerce option mapping'],
  ['CommerceError::NoVariants', 'commerce no-variants mapping'],
  ['CommerceError::ShippingProfileNotFound(_)', 'profile not-found mapping'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'profile conflict mapping'],
  ['CommerceError::Database(_)', 'profile database mapping'],
  ['CommerceError::ProductNotFound(_)', 'commerce product fallback'],
  ['CommerceError::VariantNotFound(_)', 'commerce variant fallback'],
  ['CommerceError::DuplicateHandle { .. }', 'commerce handle fallback'],
  ['CommerceError::DuplicateSku(_)', 'commerce SKU fallback'],
  ['CommerceError::InsufficientInventory { .. }', 'commerce inventory fallback'],
  ['CommerceError::CannotDeletePublished', 'commerce published fallback'],
  ['CommerceError::Rich(_)', 'commerce rich fallback'],
  ['CommerceError::Core(_)', 'commerce core fallback'],
  ['FulfillmentError::Validation(_)', 'fulfillment validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'shipping option conflict mapping'],
  ['FulfillmentError::Database(_)', 'shipping option database mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment fallback mapping'],
  ['"SHIPPING_PROFILE_REQUEST_INVALID"', 'profile validation code'],
  ['"SHIPPING_PROFILE_NOT_FOUND"', 'profile not-found code'],
  ['"SHIPPING_PROFILE_STATE_CONFLICT"', 'profile conflict code'],
  ['"SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE"', 'profile temporary code'],
  ['"SHIPPING_PROFILE_OPERATION_FAILED"', 'profile fallback code'],
  ['"SHIPPING_OPTION_REQUEST_INVALID"', 'option validation code'],
  ['"SHIPPING_OPTION_NOT_FOUND"', 'option not-found code'],
  ['"SHIPPING_OPTION_STATE_CONFLICT"', 'option conflict code'],
  ['"SHIPPING_OPTION_TEMPORARILY_UNAVAILABLE"', 'option temporary code'],
  ['"SHIPPING_OPTION_OPERATION_FAILED"', 'option fallback code'],
]) {
  requireText(facade, value, label);
}

for (const [value, label] of [
  ['use async_graphql::{Context, ErrorExtensions, Object, Result};', 'resolver Result import'],
  ['use crate::ShippingProfileService;', 'shipping profile service import'],
  ['use rustok_fulfillment::FulfillmentService;', 'fulfillment service import'],
  ['async fn create_shipping_option(', 'create shipping option mutation'],
  ['async fn update_shipping_option(', 'update shipping option mutation'],
  ['async fn deactivate_shipping_option(', 'deactivate shipping option mutation'],
  ['async fn reactivate_shipping_option(', 'reactivate shipping option mutation'],
  ['async fn create_shipping_profile(', 'create shipping profile mutation'],
  ['async fn update_shipping_profile(', 'update shipping profile mutation'],
  ['async fn deactivate_shipping_profile(', 'deactivate shipping profile mutation'],
  ['async fn reactivate_shipping_profile(', 'reactivate shipping profile mutation'],
]) {
  requireText(source, value, label);
}

const fulfillmentServiceCalls = source.match(/FulfillmentService::new\(/g) ?? [];
if (fulfillmentServiceCalls.length !== 4) {
  failures.push(`expected four shipping-option service call sites, found ${fulfillmentServiceCalls.length}`);
}
const shippingProfileServiceCalls = source.match(/ShippingProfileService::new\(/g) ?? [];
if (shippingProfileServiceCalls.length !== 4) {
  failures.push(`expected four shipping-profile service call sites, found ${shippingProfileServiceCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL checkout service error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL checkout shipping services use cloneable typed public envelopes while preserving existing GraphQL errors',
);
