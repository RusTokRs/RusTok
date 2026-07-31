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
const contextOwner = read('crates/rustok-commerce/src/services/context.rs');
const channelOwner = read('crates/rustok-channel/src/error.rs');
const channelBoundary = read('crates/rustok-commerce/src/storefront_channel.rs');
const apiPorts = read('crates/rustok-api/src/ports.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
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

const contextMapper = between(
  controller,
  'fn map_storefront_context_error(',
  'fn map_storefront_customer_port_error(',
  'storefront context mapper',
);
const customerMapper = between(
  controller,
  'fn map_storefront_customer_port_error(',
  'fn map_storefront_channel_error(',
  'storefront customer mapper',
);
const channelMapper = between(
  controller,
  'fn map_storefront_channel_error(',
  'pub fn axum_router()',
  'storefront channel mapper',
);
const contextHandler = between(
  controller,
  'pub(crate) async fn resolve_context_for_db(',
  'pub(crate) async fn resolve_context_from_cart_for_db(',
  'storefront context handler',
);
const customerHandler = between(
  controller,
  'pub(crate) async fn current_customer_id_for_db(',
  'pub(crate) fn storefront_customer_port_context(',
  'storefront customer handler',
);
const channelHandler = between(
  controller,
  'pub(crate) async fn ensure_storefront_channel_enabled_for_db(',
  'pub(crate) fn storefront_public_channel_slug_for_cart(',
  'storefront channel handler',
);

for (const [value, label] of [
  ['use axum::http::StatusCode;', 'HTTP status import'],
  ['PortActor, PortContext, PortError, RequestContext', 'typed port error import'],
  ['use rustok_channel::error::ChannelError;', 'typed channel error import'],
  ['HttpError, HttpResult, port_error_to_http_error', 'shared port mapper import'],
  ['StoreContextError, StoreContextService', 'typed context error import'],
  ['boundary = "commerce_storefront_shared_http"', 'shared boundary log'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['StoreContextError::TenantNotFound(_)', 'context tenant-not-found variant'],
  ['StoreContextError::Validation(_)', 'context validation variant'],
  ['StoreContextError::CurrencyRegionMismatch { .. }', 'context currency mismatch variant'],
  ['StoreContextError::TenantBoundary { .. }', 'context tenant boundary variant'],
  ['StoreContextError::RegionBoundary { .. }', 'context region boundary variant'],
  ['StoreContextError::Database(_)', 'context database variant'],
  ['StatusCode::NOT_FOUND', 'context not-found status'],
  ['StatusCode::BAD_REQUEST', 'context validation status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'context fail-closed status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'context unavailable status'],
  ['"commerce_store_context_not_found"', 'context not-found code'],
  ['"commerce_store_context_invalid"', 'context invalid code'],
  ['"commerce_store_context_failed"', 'context failed code'],
  ['"commerce_store_context_unavailable"', 'context unavailable code'],
  ['"tenant_boundary"', 'context tenant boundary error kind'],
  ['owner = "rustok_commerce.store_context"', 'context owner log'],
  ['operation = "resolve_store_context"', 'context operation log'],
  ['tenant_id = %tenant_id', 'context tenant log'],
  ['error = ?error', 'context raw internal error log'],
  ['HttpError::new(status, code, message)', 'context static envelope'],
]) {
  requireText(contextMapper, value, label);
}

for (const [value, label] of [
  ['let public = port_error_to_http_error(error.clone());', 'customer shared safe mapping'],
  ['owner = "rustok_customer"', 'customer owner log'],
  ['operation = "resolve_current_customer"', 'customer operation log'],
  ['tenant_id = %tenant_id', 'customer tenant log'],
  ['user_id = %user_id', 'customer user log'],
  ['error_kind = ?error.kind', 'customer typed kind log'],
  ['retryable = error.retryable', 'customer retryability log'],
  ['public_code = %public.code', 'customer public code log'],
  ['status = %public.status', 'customer public status log'],
  ['public\n}', 'customer mapped error return'],
]) {
  requireText(customerMapper, value, label);
}

for (const [value, label] of [
  ['ChannelError::NotFound(_)', 'channel not-found variant'],
  ['ChannelError::InactiveChannel(_)', 'channel inactive variant'],
  ['ChannelError::Database(_)', 'channel database variant'],
  ['ChannelError::SlugAlreadyExists(_)', 'channel duplicate slug variant'],
  ['ChannelError::InvalidTargetType(_)', 'channel target type variant'],
  ['ChannelError::InvalidTargetValue(_)', 'channel target value variant'],
  ['ChannelError::InvalidPolicyDefinition(_)', 'channel policy definition variant'],
  ['ChannelError::TargetAlreadyExists(_, _)', 'channel target conflict variant'],
  ['ChannelError::PolicySetSlugAlreadyExists(_)', 'channel policy set variant'],
  ['ChannelError::InvalidPolicyOperation(_)', 'channel policy operation variant'],
  ['ChannelError::Serialization(_)', 'channel serialization variant'],
  ['"commerce_store_channel_not_found"', 'channel not-found code'],
  ['"commerce_store_channel_unavailable"', 'channel unavailable code'],
  ['"commerce_store_channel_failed"', 'channel fail-closed code'],
  ['owner = "rustok_channel"', 'channel owner log'],
  ['operation = "ensure_storefront_channel_enabled"', 'channel operation log'],
  ['channel_id = ?request_context.channel_id', 'channel ID log'],
  ['channel_slug = ?request_context.channel_slug', 'channel slug log'],
  ['HttpError::new(status, code, message)', 'channel static envelope'],
]) {
  requireText(channelMapper, value, label);
}

for (const [value, label] of [
  ['StoreContextService::new(', 'context service construction'],
  ['rustok_region::RegionService::new(db.clone())', 'region owner construction'],
  ['ResolveStoreContextInput {', 'typed context input'],
  ['region_id,', 'context region input'],
  ['country_code,', 'context country input'],
  ['locale: locale.or_else(|| Some(request_context.locale.clone()))', 'context locale fallback'],
  ['currency_code,', 'context currency input'],
  ['map_storefront_context_error(error, tenant_id)', 'typed context mapper use'],
]) {
  requireText(contextHandler, value, label);
}
for (const value of [
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("commerce_store_invalid"',
]) {
  forbidText(contextHandler, value, 'unsafe context public conversion');
}

for (const [value, label] of [
  ['let Some(auth) = auth else', 'anonymous customer behavior'],
  ['in_process_customer_read_port(db.clone())', 'customer port construction'],
  ['read_customer_projection_by_user(', 'customer projection call'],
  ['storefront_customer_port_context(tenant_id, auth.user_id)', 'customer port context'],
  ['CustomerUserProjectionRequest {', 'typed customer request'],
  ['Ok(customer) => Ok(Some(customer.id))', 'customer identity response'],
  ['Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)', 'customer not-found fallback'],
  ['map_storefront_customer_port_error(', 'typed customer mapper use'],
]) {
  requireText(customerHandler, value, label);
}
for (const value of [
  'error.message',
  'err.to_string()',
  'HttpError::bad_request(',
]) {
  forbidText(customerHandler, value, 'unsafe customer public conversion');
}

for (const [value, label] of [
  ['is_module_enabled_for_request_channel(db, request_context, MODULE_SLUG)', 'channel module check'],
  ['map_storefront_channel_error(error, request_context)', 'typed channel mapper use'],
  ['if !enabled {', 'disabled module policy'],
  ['"commerce_store_denied"', 'disabled module public code'],
  ['request_context.channel_slug.as_deref().unwrap_or("current")', 'disabled module channel context'],
]) {
  requireText(channelHandler, value, label);
}
for (const value of [
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("commerce_store_invalid"',
]) {
  forbidText(channelHandler, value, 'unsafe channel public conversion');
}

for (const [content, value, label] of [
  [contextOwner, 'pub enum StoreContextError {', 'context owner enum'],
  [contextOwner, 'TenantNotFound(Uuid)', 'context owner tenant variant'],
  [contextOwner, 'Validation(String)', 'context owner validation variant'],
  [contextOwner, 'CurrencyRegionMismatch {', 'context owner currency variant'],
  [contextOwner, 'TenantBoundary { code: String, message: String }', 'context owner tenant variant'],
  [contextOwner, 'RegionBoundary { code: String, message: String }', 'context owner region variant'],
  [contextOwner, 'Database(#[from] sea_orm::DbErr)', 'context owner database variant'],
  [channelOwner, 'pub enum ChannelError {', 'channel owner enum'],
  [channelOwner, 'SlugAlreadyExists(String)', 'channel owner duplicate slug variant'],
  [channelOwner, 'NotFound(Uuid)', 'channel owner not-found variant'],
  [channelOwner, 'InactiveChannel(Uuid)', 'channel owner inactive variant'],
  [channelOwner, 'InvalidTargetType(String)', 'channel owner target type variant'],
  [channelOwner, 'InvalidTargetValue(String)', 'channel owner target value variant'],
  [channelOwner, 'InvalidPolicyDefinition(String)', 'channel owner policy definition variant'],
  [channelOwner, 'TargetAlreadyExists(String, String)', 'channel owner target conflict variant'],
  [channelOwner, 'PolicySetSlugAlreadyExists(String)', 'channel owner policy set variant'],
  [channelOwner, 'InvalidPolicyOperation(String)', 'channel owner operation variant'],
  [channelOwner, 'Database(#[from] DbErr)', 'channel owner database variant'],
  [channelOwner, 'Serialization(#[from] SerdeJsonError)', 'channel owner serialization variant'],
  [channelBoundary, '-> Result<bool, ChannelError>', 'typed channel boundary result'],
  [channelBoundary, '.is_module_enabled(channel_id, module_slug)', 'channel service operation'],
  [apiPorts, 'pub struct PortError {', 'owner port error type'],
  [apiPorts, 'pub kind: PortErrorKind', 'owner port kind'],
  [apiPorts, 'pub retryable: bool', 'owner port retryability'],
  [webErrors, 'pub fn port_error_to_http_error(error: PortError)', 'shared port HTTP mapper'],
  [webErrors, 'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE', 'shared unavailable status'],
  [webErrors, 'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT', 'shared timeout status'],
  [webErrors, 'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR', 'shared invariant status'],
  [webErrors, '"The requested service is temporarily unavailable"', 'safe unavailable message'],
  [webErrors, '"The requested operation could not be completed"', 'safe invariant message'],
]) {
  requireText(content, value, label);
}

for (const [value, expected, label] of [
  ['map_storefront_context_error(', 2, 'context mapper definition and use'],
  ['map_storefront_customer_port_error(', 2, 'customer mapper definition and use'],
  ['map_storefront_channel_error(', 2, 'channel mapper definition and use'],
]) {
  const count = controller.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['pub fn axum_router()', 'storefront router'],
  ['pub(crate) async fn resolve_context_from_cart_for_db(', 'cart context wrapper'],
  ['pub(crate) async fn ensure_customer_owns_order_for_db(', 'order ownership helper'],
  ['pub(crate) fn storefront_customer_port_context(', 'customer port context helper'],
  ['pub(crate) fn storefront_cart_port_context(', 'cart port context helper'],
  ['pub(crate) fn ensure_store_cart_access(', 'cart access helper'],
  ['pub(crate) async fn apply_cart_context_patch_for_db(', 'cart patch helper'],
  ['pub(crate) async fn enrich_storefront_cart_for_db(', 'cart enrichment helper'],
  ['pub struct StoreListProductsParams', 'product params DTO'],
  ['pub struct StoreCreateCartInput', 'create cart DTO'],
  ['pub struct StoreCompleteCartInput', 'checkout DTO'],
  ['pub struct StoreCartShippingSelectionInput', 'shipping selection DTO'],
]) {
  requireText(controller, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront shared HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Shared storefront context, customer, and channel errors use typed safe envelopes');
