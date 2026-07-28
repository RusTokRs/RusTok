#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { readCommerceSafeQuerySource } from './lib/commerce-safe-query-source.mjs';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const serverRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const adminRest = read('crates/rustok-commerce/src/controllers/admin/shipping.rs');
const storefrontRest = read('crates/rustok-commerce/src/controllers/store/products.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const safeQuery = readCommerceSafeQuerySource(read);
const commerceStorefrontTransport = read('crates/rustok-commerce/storefront/src/transport/mod.rs');
const commerceNativeAdapter = read(
  'crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs',
);
const fulfillmentStorefrontTransport = read('crates/rustok-fulfillment/storefront/src/transport.rs');
const evidence = JSON.parse(
  read(
    'crates/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json',
  ),
);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
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

const adminList = between(
  adminRest,
  'pub async fn list_shipping_options(',
  '/// Create admin shipping option',
  'admin shipping-option list',
);
const adminLookup = between(
  adminRest,
  'pub async fn show_shipping_option(',
  '/// Update admin shipping option',
  'admin shipping-option lookup',
);
const storefrontList = storefrontRest.slice(
  storefrontRest.indexOf('pub async fn list_shipping_options('),
);
if (!storefrontList) failures.push('storefront shipping-option list: unable to isolate source block');

for (const [source, value, label] of [
  [serverRuntime, 'CommerceShippingOptionReadRuntime::in_process(', 'host read runtime factory'],
  [serverRuntime, 'host.with_shared_value(runtime)', 'host read runtime attachment'],
  [graphqlRuntime, 'pub struct CommerceShippingOptionReadRuntime', 'typed shared read runtime'],
  [safeQuery, 'shipping_option_runtime.shipping_option_read_port()', 'GraphQL storefront port'],
  [
    safeQuery,
    'shipping_option_runtime\n                .shipping_option_admin_read_port()',
    'GraphQL admin port',
  ],
  [httpRuntime, 'shipping_option_read_runtime: crate::graphql_runtime::CommerceShippingOptionReadRuntime', 'HTTP runtime field'],
  [httpRuntime, 'shipping_option_read_port(', 'HTTP storefront getter'],
  [httpRuntime, 'shipping_option_admin_read_port(', 'HTTP admin getter'],
  [
    httpRuntime,
    'Commerce HTTP routes require CommerceShippingOptionReadRuntime in HostRuntimeContext',
    'HTTP fail-closed host requirement',
  ],
]) {
  requireText(source, value, label);
}

for (const [source, value, label] of [
  [adminRest, 'fn admin_shipping_option_read_port_context(', 'admin read context builder'],
  [adminRest, 'PortActor::user(auth.user_id.to_string())', 'admin user actor'],
  [adminRest, 'request_context.locale.as_str()', 'admin locale context'],
  [adminRest, 'request_context.channel_slug.as_deref()', 'admin channel propagation'],
  [adminRest, 'with_deadline(std::time::Duration::from_secs(2))', 'admin read deadline'],
  [adminRest, 'fn map_admin_shipping_option_port_error(', 'admin typed port mapper'],
  [adminRest, 'PortErrorKind::Validation', 'admin validation mapping'],
  [adminRest, 'PortErrorKind::NotFound', 'admin not-found mapping'],
  [adminRest, 'PortErrorKind::Conflict', 'admin conflict mapping'],
  [adminRest, 'PortErrorKind::Forbidden', 'admin forbidden mapping'],
  [adminRest, 'PortErrorKind::Unavailable | PortErrorKind::Timeout', 'admin unavailable mapping'],
  [adminRest, 'PortErrorKind::InvariantViolation', 'admin invariant mapping'],
  [adminList, '.shipping_option_admin_read_port()', 'admin host-composed port'],
  [adminList, '.list_all_shipping_option_projections(', 'admin owner operation'],
  [adminLookup, '.shipping_option_read_port()', 'admin lookup host-composed port'],
  [adminLookup, '.read_shipping_option_projection(', 'admin lookup owner operation'],
]) {
  requireText(source, value, label);
}

for (const value of ['FulfillmentService::new(', '.list_all_shipping_options(', '.get_shipping_option(']) {
  forbidText(adminList + adminLookup, value, 'admin read handlers must not construct concrete fulfillment reads');
}

for (const [value, label] of [
  ['items.retain(|option| option.active == active);', 'admin active filter'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'admin currency filter'],
  ['option.provider_id.eq_ignore_ascii_case(provider_id)', 'admin provider filter'],
  ['option.name.to_ascii_lowercase().contains(&search)', 'admin search filter'],
  ['skip(pagination.offset() as usize)', 'admin pagination offset'],
  ['take(pagination.limit() as usize)', 'admin pagination limit'],
]) {
  requireText(adminList, value, label);
}

for (const [source, value, label] of [
  [storefrontRest, 'fn storefront_shipping_option_port_context(', 'storefront read context builder'],
  [storefrontRest, 'PortActor::service("rustok-commerce.storefront-shipping-options")', 'storefront anonymous actor'],
  [storefrontRest, 'PortActor::user(value.user_id.to_string())', 'storefront user actor'],
  [storefrontRest, 'public_channel_slug: Option<&str>', 'storefront effective channel input'],
  [storefrontRest, 'with_deadline(std::time::Duration::from_secs(2))', 'storefront read deadline'],
  [storefrontRest, 'fn map_storefront_shipping_port_error(', 'storefront typed port mapper'],
  [storefrontList, '.shipping_option_read_port()', 'storefront host-composed port'],
  [storefrontList, '.list_shipping_option_projections(', 'storefront owner operation'],
  [storefrontList, 'requested_locale: Some(request_context.locale.clone())', 'storefront requested locale'],
  [storefrontList, 'tenant_default_locale: Some(tenant.default_locale.clone())', 'storefront fallback locale'],
  [storefrontList, 'public_channel_slug.as_deref()', 'storefront effective channel propagation'],
  [storefrontList, 'option.currency_code.eq_ignore_ascii_case(currency_code)', 'storefront currency filter'],
  [storefrontList, 'is_metadata_visible_for_public_channel(', 'storefront channel filter'],
  [storefrontList, 'is_shipping_option_compatible_with_profiles(', 'storefront profile filter'],
]) {
  requireText(source, value, label);
}
for (const value of ['FulfillmentService::new(', '.list_shipping_options(']) {
  forbidText(storefrontList, value, 'storefront read handler must not construct concrete fulfillment reads');
}

for (const [source, value, label] of [
  [commerceStorefrontTransport, 'select_storefront_shipping_option(', 'Commerce selection handoff'],
  [
    commerceStorefrontTransport,
    'rustok_fulfillment_storefront::transport::select_shipping_option',
    'fulfillment selection transport',
  ],
  [fulfillmentStorefrontTransport, 'pub async fn select_shipping_option(', 'selection transport API'],
  [fulfillmentStorefrontTransport, 'ShippingSelectionDeliveryGroup', 'selection projection'],
  [fulfillmentStorefrontTransport, 'UiTransportPath::NativeServer', 'native selection path'],
  [fulfillmentStorefrontTransport, 'UiTransportPath::Graphql', 'GraphQL selection path'],
]) {
  requireText(source, value, label);
}
for (const value of [
  'list_shipping_option_projections',
  'list_all_shipping_option_projections',
  'read_shipping_option_projection',
]) {
  forbidText(
    commerceStorefrontTransport + commerceNativeAdapter + fulfillmentStorefrontTransport,
    value,
    'native FFA must not invent a projection-read surface',
  );
}

const expectedEvidence = {
  status: 'source_cutover_ready_unvalidated',
  sharedComposition: 'application_host',
  restComposition: 'application_host',
  concreteReadConstruction: false,
  restCutoverRequired: false,
  nativeProjectionSurface: 'absent_by_design',
  nextSlice: 'execute_mounted_graphql_rest_projection_parity',
  runtimeParityProven: false,
};
const actualEvidence = {
  status: evidence.status,
  sharedComposition: evidence.shared_runtime?.composition,
  restComposition: evidence.rest?.composition,
  concreteReadConstruction: evidence.rest?.concrete_service_read_construction,
  restCutoverRequired: evidence.rest?.cutover_required,
  nativeProjectionSurface: evidence.native_ffa?.projection_read_surface,
  nextSlice: evidence.decision?.next_slice,
  runtimeParityProven: evidence.runtime_parity_proven,
};
if (JSON.stringify(actualEvidence) !== JSON.stringify(expectedEvidence)) {
  failures.push(
    `source evidence mismatch: expected ${JSON.stringify(expectedEvidence)}, received ${JSON.stringify(actualEvidence)}`,
  );
}
if (evidence.native_ffa?.expand_selection_contract_for_projection_parity !== false) {
  failures.push('source evidence must keep selection and projection-read contracts separate');
}
for (const field of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
]) {
  if (evidence.validation?.[field] !== false) {
    failures.push(`source evidence validation.${field} must remain false for this unvalidated cutover`);
  }
}

if (failures.length > 0) {
  console.error('Commerce shipping-option transport parity inventory verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Shipping-option GraphQL and REST projection reads share the host-composed owner runtime while native seller/cart selection remains separate',
);
