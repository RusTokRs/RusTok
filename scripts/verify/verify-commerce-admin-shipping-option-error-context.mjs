#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shipping = read('crates/rustok-commerce/src/controllers/admin/shipping.rs');
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
const requireBefore = (content, first, second, label) => {
  const firstIndex = content.indexOf(first);
  const secondIndex = content.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    failures.push(`${label}: ${first} must precede ${second}`);
  }
};

const diagnosticProjection = between(
  shipping,
  'struct AdminShippingOptionDiagnosticContext {',
  'fn map_shipping_profile_error(',
  'admin shipping diagnostic projection',
);
const mutationMapper = between(
  shipping,
  'fn map_admin_shipping_option_error(',
  'fn admin_shipping_option_read_port_context(',
  'admin shipping-option mutation mapper',
);
const readContextBuilder = between(
  shipping,
  'fn admin_shipping_option_read_port_context(',
  'fn map_admin_shipping_option_port_error(',
  'admin shipping-option read context builder',
);
const portMapper = between(
  shipping,
  'fn map_admin_shipping_option_port_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'admin shipping-option port mapper',
);
const listRoute = between(
  shipping,
  'pub async fn list_shipping_options(',
  '/// Create admin shipping option',
  'list shipping options route',
);
const createRoute = between(
  shipping,
  'pub async fn create_shipping_option(',
  '/// Show admin shipping option',
  'create shipping option route',
);
const showRoute = between(
  shipping,
  'pub async fn show_shipping_option(',
  '/// Update admin shipping option',
  'show shipping option route',
);
const updateRoute = between(
  shipping,
  'pub async fn update_shipping_option(',
  '/// Deactivate admin shipping option',
  'update shipping option route',
);
const deactivateRoute = between(
  shipping,
  'pub async fn deactivate_shipping_option(',
  '/// Reactivate admin shipping option',
  'deactivate shipping option route',
);
const reactivateStart = shipping.indexOf('pub async fn reactivate_shipping_option(');
const reactivateRoute = reactivateStart < 0 ? '' : shipping.slice(reactivateStart);
if (reactivateStart < 0) {
  failures.push('reactivate shipping option route: unable to isolate source block');
}

for (const [value, label] of [
  ['struct AdminShippingOptionErrorContext {', 'typed error context'],
  ['tenant_id: Uuid,', 'typed tenant identity'],
  ['shipping_option_id: Option<Uuid>,', 'typed option identity'],
  ["operation: &'static str,", 'typed operation'],
  ['struct AdminShippingOptionDiagnosticContext {', 'bounded route diagnostic context'],
  ['struct AdminShippingOptionPortDiagnosticContext {', 'bounded port diagnostic context'],
  ['struct AdminShippingOptionPortDiagnosticError', 'bounded port diagnostic error'],
  ['struct AdminShippingDiagnosticError;', 'bounded owner diagnostic error'],
  ['formatter.write_str("redacted")', 'redacted Debug output'],
  ["fn uuid_shape(value: Uuid) -> &'static str", 'required UUID shape helper'],
  ["fn optional_uuid_shape(value: Option<Uuid>) -> &'static str", 'optional UUID shape helper'],
  ["fn text_presence_shape(value: &str) -> &'static str", 'text presence helper'],
  ["fn optional_text_presence_shape(value: Option<&str>) -> &'static str", 'optional text presence helper'],
  ['"nil"', 'nil UUID shape'],
  ['"non_nil"', 'non-nil UUID shape'],
  ['"absent"', 'absent optional shape'],
  ['"present_nil"', 'present nil optional shape'],
  ['"present_non_nil"', 'present non-nil optional shape'],
  ['"empty"', 'empty text shape'],
  ['"present_empty"', 'present empty text shape'],
  ['"present_non_empty"', 'present non-empty text shape'],
]) requireText(shipping, value, label);

for (const [value, label] of [
  ['tenant_id: uuid_shape(context.tenant_id)', 'tenant projection'],
  ['shipping_option_id: optional_uuid_shape(context.shipping_option_id)', 'option projection'],
  ['operation: context.operation', 'operation projection'],
  ['correlation_id: text_presence_shape(context.correlation_id.as_str())', 'correlation projection'],
  ['actor: text_presence_shape(context.actor.id.as_str())', 'actor projection'],
  ['channel: optional_text_presence_shape(context.channel.as_deref())', 'channel projection'],
  ['locale: context.locale.len()', 'locale projection'],
  ['deadline_ms: context.deadline_ms', 'deadline projection'],
]) requireText(diagnosticProjection, value, label);

for (const [value, label] of [
  ['PortActor::user(auth.user_id.to_string())', 'authenticated actor'],
  ['request_context.locale.as_str()', 'request locale'],
  ['format!("commerce-admin-shipping-option:{operation}:{resource_id}")', 'resource correlation id'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['request_context.channel_slug.as_deref()', 'channel propagation'],
]) requireText(readContextBuilder, value, label);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'mutation validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'mutation option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'mutation fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'mutation conflict variant'],
  ['FulfillmentError::Database(_)', 'mutation database variant'],
  ['let context = AdminShippingOptionDiagnosticContext::from(&context);', 'mutation context shadow'],
  ['let error = AdminShippingDiagnosticError;', 'mutation error shadow'],
  ['error = ?error', 'redacted mutation error event'],
  ['tenant_id = %context.tenant_id', 'bounded mutation tenant event'],
  ['shipping_option_id = ?context.shipping_option_id', 'bounded mutation option event'],
  ['HttpError::new(status, code, message)', 'mutation static envelope'],
]) requireText(mutationMapper, value, label);
requireBefore(
  mutationMapper,
  'FulfillmentError::Validation(_)',
  'let context = AdminShippingOptionDiagnosticContext::from(&context);',
  'mutation typed policy selection',
);
requireBefore(
  mutationMapper,
  'let error = AdminShippingDiagnosticError;',
  'tracing::error!(',
  'mutation diagnostic shadow',
);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'port validation kind'],
  ['PortErrorKind::NotFound', 'port not-found kind'],
  ['PortErrorKind::Conflict', 'port conflict kind'],
  ['PortErrorKind::Forbidden', 'port forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'port unavailable kinds'],
  ['PortErrorKind::InvariantViolation', 'port invariant kind'],
  ['let context = AdminShippingOptionDiagnosticContext::from(&context);', 'port route context shadow'],
  ['let port_context = AdminShippingOptionPortDiagnosticContext::from(port_context);', 'port metadata shadow'],
  ['let error = AdminShippingOptionPortDiagnosticError {', 'port error shadow'],
  ['code: error.code.as_str()', 'stable internal code capture'],
  ['retryable: error.retryable', 'retryability capture'],
  ['correlation_id = %port_context.correlation_id', 'bounded correlation event'],
  ['actor = ?port_context.actor', 'bounded actor event'],
  ['channel = ?port_context.channel', 'bounded channel event'],
  ['locale = %port_context.locale', 'bounded locale event'],
  ['deadline_ms = ?port_context.deadline_ms', 'deadline event'],
  ['internal_code = %error.code', 'stable code event'],
  ['retryable = error.retryable', 'retryability event'],
  ['HttpError::new(status, code, message)', 'port static envelope'],
]) requireText(portMapper, value, label);
requireBefore(
  portMapper,
  'PortErrorKind::Validation',
  'let context = AdminShippingOptionDiagnosticContext::from(&context);',
  'port typed policy selection',
);
requireBefore(
  portMapper,
  'let error = AdminShippingOptionPortDiagnosticError {',
  'tracing::error!(',
  'port diagnostic shadow',
);

for (const [block, operation, request, label] of [
  [
    listRoute,
    '.list_all_shipping_option_projections(',
    'ListAllShippingOptionProjectionsRequest {',
    'list route',
  ],
  [
    showRoute,
    '.read_shipping_option_projection(',
    'ReadShippingOptionProjectionRequest {',
    'show route',
  ],
]) {
  requireText(block, 'admin_shipping_option_read_port_context(', `${label} context construction`);
  requireText(block, operation, `${label} owner operation`);
  requireText(block, request, `${label} typed request`);
  requireText(block, 'map_admin_shipping_option_port_error(', `${label} typed mapper`);
  requireText(block, 'requested_locale: Some(request_context.locale.clone())', `${label} locale`);
  requireText(block, 'tenant_default_locale: Some(tenant.default_locale.clone())', `${label} fallback locale`);
  forbidText(block, 'FulfillmentService::new(', `${label} concrete service`);
}

for (const [block, operation, label] of [
  [createRoute, '.create_shipping_option(tenant.id, input)', 'create route'],
  [updateRoute, '.update_shipping_option(tenant.id, id, input)', 'update route'],
  [deactivateRoute, '.deactivate_shipping_option(tenant.id, id)', 'deactivate route'],
  [reactivateRoute, '.reactivate_shipping_option(tenant.id, id)', 'reactivate route'],
]) {
  requireText(block, 'FulfillmentService::new(runtime.db_clone())', `${label} lifecycle service`);
  requireText(block, operation, `${label} service operation`);
  requireText(block, 'map_admin_shipping_option_error(', `${label} typed mapper`);
}

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  '.map_err(super::map_fulfillment_error)?;',
]) forbidText(shipping, value, 'unsafe admin shipping-option public conversion');

const mutationMapperUses = shipping.match(/map_admin_shipping_option_error\(/g) ?? [];
if (mutationMapperUses.length !== 5) {
  failures.push(`expected mutation mapper definition plus four uses, found ${mutationMapperUses.length}`);
}
const portMapperUses = shipping.match(/map_admin_shipping_option_port_error\(/g) ?? [];
if (portMapperUses.length !== 3) {
  failures.push(`expected port mapper definition plus two read uses, found ${portMapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce admin shipping-option error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping-option reads and mutations retain typed policy while emitting only bounded diagnostics',
);
