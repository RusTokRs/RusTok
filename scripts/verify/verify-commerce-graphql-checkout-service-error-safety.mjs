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
  ['use ::rustok_api::{PortContext, PortError, PortErrorKind};', 'typed owner port errors'],
  ['const CHECKOUT_ERROR_BOUNDARY: &str = "commerce_graphql_checkout";', 'checkout boundary constant'],
  ['struct CheckoutServiceDiagnosticError;', 'redacted diagnostic token'],
  ['impl std::fmt::Debug for CheckoutServiceDiagnosticError', 'diagnostic Debug implementation'],
  ['formatter.write_str("redacted")', 'redacted diagnostic rendering'],
  ['#[derive(Clone)]', 'async GraphQL clone requirement'],
  ['pub(crate) enum BoundaryError {', 'local boundary error'],
  ['Graphql(Error)', 'GraphQL pass-through variant'],
  ['Public {', 'cloneable public envelope variant'],
  ['impl From<Error> for BoundaryError', 'GraphQL conversion'],
  ['impl From<CommerceError> for BoundaryError', 'commerce conversion'],
  ['pub(crate) fn shipping_option_port_error(', 'shipping option port mapper'],
  ['fn shipping_option_port_error_envelope(', 'shipping option envelope mapper'],
  ['PortErrorKind::Validation', 'shipping option validation mapping'],
  ['PortErrorKind::NotFound if error.code == "fulfillment.shipping_option_not_found"', 'shipping option not-found mapping'],
  ['PortErrorKind::Conflict', 'shipping option conflict mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'shipping option temporary mapping'],
  ['PortErrorKind::Forbidden', 'shipping option forbidden fallback'],
  ['PortErrorKind::InvariantViolation', 'shipping option invariant fallback'],
  ['owner = "rustok_fulfillment.shipping_option_admin_command"', 'shipping option owner diagnostic'],
  ['owner_operation,', 'shipping option owner operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind diagnostic'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code diagnostic'],
  ['extensions.set("code", code)', 'stable code extension'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
  ['impl From<BoundaryError> for Error', 'public GraphQL conversion'],
  ['BoundaryError::Graphql(error) => error', 'existing GraphQL error preservation'],
  ['mod async_graphql_shim {', 'async GraphQL result shim'],
  [
    'pub type Result<T> = std::result::Result<T, super::checkout_boundary::BoundaryError>;',
    'custom checkout result',
  ],
  ['include!("checkout.rs");', 'checkout resolver inclusion'],
]) {
  requireText(facade, value, label);
}

for (const value of [
  'FulfillmentError',
  'error = ?error',
  'error = %error',
  'owner_message = %error.message',
  'message = %error.message',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(facade, value, 'checkout facade public and diagnostic boundary');
}

for (const [value, label] of [
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
  ['AuthContext, PortActor, PortContext, RequestContext', 'trusted owner call facts'],
  ['CreateAdminShippingOptionRequest', 'create owner request'],
  ['UpdateAdminShippingOptionRequest', 'update owner request'],
  ['DeactivateAdminShippingOptionRequest', 'deactivate owner request'],
  ['ReactivateAdminShippingOptionRequest', 'reactivate owner request'],
  ['fn shipping_option_command_context(', 'shipping option command context'],
  ['PortActor::user(auth.user_id.to_string())', 'authenticated owner actor'],
  ['.with_idempotency_key(Uuid::new_v4().to_string())', 'ephemeral write identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'owner deadline'],
  ['request.channel_slug.as_deref()', 'owner channel propagation'],
  ['shipping_option_admin_command_runtime_from_context(', 'host-selected owner runtime'],
  ['.create_shipping_option(command_context.clone(), request)', 'create owner call'],
  ['.update_shipping_option(command_context.clone(), request)', 'update owner call'],
  ['.deactivate_shipping_option(command_context.clone(), request)', 'deactivate owner call'],
  ['.reactivate_shipping_option(command_context.clone(), request)', 'reactivate owner call'],
  ['checkout_boundary::shipping_option_port_error(', 'bounded owner error mapper'],
  ['use crate::ShippingProfileService;', 'shipping profile service import'],
  ['async fn create_shipping_profile(', 'create shipping profile mutation'],
  ['async fn update_shipping_profile(', 'update shipping profile mutation'],
  ['async fn deactivate_shipping_profile(', 'deactivate shipping profile mutation'],
  ['async fn reactivate_shipping_profile(', 'reactivate shipping profile mutation'],
]) {
  requireText(source, value, label);
}

for (const value of [
  'use rustok_fulfillment::FulfillmentService;',
  'FulfillmentService::new(',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(err.to_string())',
  'Error::new(error.to_string())',
  'Error::new(err.to_string())',
  'format!("{error}")',
]) {
  forbidText(source, value, 'checkout resolver direct owner/public boundary');
}

const ownerRuntimeCalls = source.match(/shipping_option_admin_command_runtime_from_context\(/g) ?? [];
if (ownerRuntimeCalls.length !== 4) {
  failures.push(`expected four shipping-option owner runtime call sites, found ${ownerRuntimeCalls.length}`);
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
  '✔ Commerce GraphQL shipping-option writes use the owner command port with bounded errors while Commerce-owned shipping-profile envelopes remain intact',
);
