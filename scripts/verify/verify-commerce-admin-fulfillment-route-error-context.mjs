#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-commerce/src/controllers/admin/fulfillments.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
const orchestrationErrors = read(
  'crates/rustok-commerce/src/services/fulfillment_orchestration.rs',
);
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
const requireShadowBeforeTrace = (content, shadow, label) => {
  const shadowIndex = content.indexOf(shadow);
  const traceIndex = content.indexOf('tracing::error!(');
  if (shadowIndex < 0 || traceIndex < 0 || shadowIndex > traceIndex) {
    failures.push(`${label}: diagnostic shadow must precede tracing::error!`);
  }
};

const portMapper = between(
  source,
  'fn map_admin_fulfillment_port_error(',
  '/// List admin fulfillments',
  'admin fulfillment port mapper',
);
const ownerMapper = between(
  source,
  'fn map_admin_fulfillment_error(',
  'fn map_admin_fulfillment_orchestration_error(',
  'admin fulfillment owner mapper',
);
const orchestrationStart = source.indexOf('fn map_admin_fulfillment_orchestration_error(');
const orchestrationMapper =
  orchestrationStart < 0 ? '' : source.slice(orchestrationStart);
if (orchestrationStart < 0) {
  failures.push('admin fulfillment orchestration mapper: unable to isolate source block');
}

for (const [value, label] of [
  ['struct AdminFulfillmentErrorContext {', 'typed route error context'],
  ['tenant_id: Uuid,', 'typed tenant identity'],
  ['fulfillment_id: Option<Uuid>,', 'typed fulfillment identity'],
  ['order_id: Option<Uuid>,', 'typed order identity'],
  ["operation: &'static str,", 'typed operation'],
  ['struct AdminFulfillmentDiagnosticContext {', 'bounded route diagnostic context'],
  ["tenant_id: &'static str,", 'bounded tenant shape'],
  ["fulfillment_id: &'static str,", 'bounded fulfillment shape'],
  ["order_id: &'static str,", 'bounded order shape'],
  ['impl From<&AdminFulfillmentErrorContext> for AdminFulfillmentDiagnosticContext', 'typed-to-diagnostic conversion'],
  ['struct AdminFulfillmentPortDiagnosticContext {', 'bounded port diagnostic context'],
  ["correlation_id: &'static str,", 'bounded correlation shape'],
  ["actor: &'static str,", 'bounded actor shape'],
  ["channel: &'static str,", 'bounded channel shape'],
  ['locale: usize,', 'bounded locale shape'],
  ['deadline_ms: Option<u64>,', 'deadline retention'],
  ['struct AdminFulfillmentPortDiagnosticError', 'bounded port diagnostic error'],
  ['struct AdminFulfillmentDiagnosticError;', 'bounded owner diagnostic error'],
  ['formatter.write_str("redacted")', 'redacted Debug output'],
  ["fn uuid_shape(value: Uuid) -> &'static str", 'required UUID shape helper'],
  ["fn optional_uuid_shape(value: Option<Uuid>) -> &'static str", 'optional UUID shape helper'],
  ["fn text_presence_shape(value: &str) -> &'static str", 'text presence shape helper'],
  ["fn optional_text_presence_shape(value: Option<&str>) -> &'static str", 'optional text presence shape helper'],
  ['"nil"', 'nil UUID shape'],
  ['"non_nil"', 'non-nil UUID shape'],
  ['"absent"', 'absent optional shape'],
  ['"present_nil"', 'present nil optional shape'],
  ['"present_non_nil"', 'present non-nil optional shape'],
  ['"empty"', 'empty text shape'],
  ['"present_empty"', 'present empty text shape'],
  ['"present_non_empty"', 'present non-empty text shape'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'port validation policy'],
  ['PortErrorKind::NotFound', 'port not-found policy'],
  ['PortErrorKind::Conflict', 'port conflict policy'],
  ['PortErrorKind::Forbidden', 'port forbidden policy'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'port unavailable policy'],
  ['PortErrorKind::InvariantViolation', 'port invariant policy'],
  ['let context = AdminFulfillmentDiagnosticContext::from(&context);', 'port context shadow'],
  ['let port_context = AdminFulfillmentPortDiagnosticContext::from(port_context);', 'port metadata shadow'],
  ['let error = AdminFulfillmentPortDiagnosticError {', 'port error shadow'],
  ['code: error.code.as_str()', 'stable internal code retention'],
  ['retryable: error.retryable', 'retryability retention'],
  ['error = ?error', 'redacted port diagnostic'],
  ['owner = ADMIN_FULFILLMENT_OWNER', 'port owner'],
  ['owner_operation,', 'port owner operation'],
  ['correlation_id = %port_context.correlation_id', 'correlation shape log'],
  ['tenant_id = %context.tenant_id', 'tenant shape log'],
  ['fulfillment_id = ?context.fulfillment_id', 'fulfillment shape log'],
  ['order_id = ?context.order_id', 'order shape log'],
  ['operation = %context.operation', 'operation log'],
  ['actor = ?port_context.actor', 'actor shape log'],
  ['channel = ?port_context.channel', 'channel shape log'],
  ['locale = %port_context.locale', 'locale length log'],
  ['deadline_ms = ?port_context.deadline_ms', 'deadline log'],
  ['internal_code = %error.code', 'stable internal code log'],
  ['retryable = error.retryable', 'retryability log'],
  ['HttpError::new(status, code, message)', 'static port envelope'],
]) requireText(portMapper, value, label);
requireShadowBeforeTrace(
  portMapper,
  'let error = AdminFulfillmentPortDiagnosticError {',
  'admin fulfillment port mapper',
);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'owner validation policy'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'owner shipping-option policy'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'owner fulfillment policy'],
  ['FulfillmentError::InvalidTransition { .. }', 'owner transition policy'],
  ['FulfillmentError::Database(_)', 'owner database policy'],
  ['let context = AdminFulfillmentDiagnosticContext::from(&context);', 'owner context shadow'],
  ['let error = AdminFulfillmentDiagnosticError;', 'owner error shadow'],
  ['error = ?error', 'redacted owner diagnostic'],
  ['owner = ADMIN_FULFILLMENT_OWNER', 'owner diagnostic owner'],
  ['HttpError::new(status, code, message)', 'static owner envelope'],
]) requireText(ownerMapper, value, label);
requireShadowBeforeTrace(
  ownerMapper,
  'let error = AdminFulfillmentDiagnosticError;',
  'admin fulfillment owner mapper',
);

for (const [value, label] of [
  ['FulfillmentOrchestrationError::Fulfillment(error)', 'nested owner delegation'],
  ['FulfillmentOrchestrationError::OrderNotFound(_)', 'order-not-found policy'],
  ['FulfillmentOrchestrationError::Database(_)', 'database policy'],
  ['FulfillmentOrchestrationError::Validation(_)', 'validation policy'],
  ['FulfillmentOrchestrationError::ProviderAfterPersistence {', 'provider-after-persistence policy'],
  ['FulfillmentOrchestrationError::PersistenceAfterProvider {', 'persistence-after-provider policy'],
  ['context.fulfillment_id = Some(*fulfillment_id);', 'reconciliation fulfillment identity adoption'],
  ['let context = AdminFulfillmentDiagnosticContext::from(&context);', 'orchestration context shadow'],
  ['let error = AdminFulfillmentDiagnosticError;', 'orchestration error shadow'],
  ['owner = ADMIN_FULFILLMENT_ORCHESTRATION_OWNER', 'orchestration owner'],
  ['"Fulfillment operation requires reconciliation"', 'static reconciliation envelope'],
  ['HttpError::new(status, code, message)', 'static orchestration envelope'],
]) requireText(orchestrationMapper, value, label);
requireShadowBeforeTrace(
  orchestrationMapper,
  'let error = AdminFulfillmentDiagnosticError;',
  'admin fulfillment orchestration mapper',
);

for (const [value, label] of [
  ['pub async fn list_fulfillments(', 'list route'],
  ['pub async fn create_fulfillment(', 'create route'],
  ['pub async fn show_fulfillment(', 'show route'],
  ['pub async fn ship_fulfillment(', 'ship route'],
  ['pub async fn deliver_fulfillment(', 'deliver route'],
  ['pub async fn reopen_fulfillment(', 'reopen route'],
  ['pub async fn reship_fulfillment(', 'reship route'],
  ['pub async fn cancel_fulfillment(', 'cancel route'],
  ['[Permission::FULFILLMENTS_READ]', 'read permission'],
  ['[Permission::FULFILLMENTS_CREATE]', 'create permission'],
  ['[Permission::FULFILLMENTS_UPDATE]', 'update permission'],
  ['.list_fulfillment_projections(', 'list owner-port call'],
  ['.read_fulfillment_projection(', 'detail owner-port call'],
  ['.create_manual_fulfillment(tenant.id, input)', 'create orchestration call'],
  ['.ship_fulfillment(tenant.id, id, input)', 'ship orchestration call'],
  ['.deliver_fulfillment(tenant.id, id, input)', 'deliver owner call'],
  ['.reopen_fulfillment(tenant.id, id, input)', 'reopen owner call'],
  ['.reship_fulfillment(tenant.id, id, input)', 'reship orchestration call'],
  ['.cancel_fulfillment(tenant.id, id, input)', 'cancel orchestration call'],
]) requireText(source, value, label);

const portMapperUses =
  source.match(/map_admin_fulfillment_port_error\(\s+AdminFulfillmentErrorContext::new\(/g) ?? [];
if (portMapperUses.length !== 2) {
  failures.push(`expected two context-aware port mapper callsites, found ${portMapperUses.length}`);
}
const ownerMapperUses =
  source.match(/map_admin_fulfillment_error\(\s+AdminFulfillmentErrorContext::new\(/g) ?? [];
if (ownerMapperUses.length !== 2) {
  failures.push(`expected two context-aware owner mapper callsites, found ${ownerMapperUses.length}`);
}
const orchestrationMapperUses =
  source.match(/map_admin_fulfillment_orchestration_error\(\s+AdminFulfillmentErrorContext::new\(/g) ?? [];
if (orchestrationMapperUses.length !== 4) {
  failures.push(
    `expected four context-aware orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
}

for (const [ownerSource, value, label] of [
  [fulfillmentErrors, 'Validation(String)', 'owner validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner database variant'],
  [orchestrationErrors, 'OrderNotFound(Uuid)', 'orchestration order-not-found variant'],
  [orchestrationErrors, 'Database(#[from] sea_orm::DbErr)', 'orchestration database variant'],
  [orchestrationErrors, 'Validation(String)', 'orchestration validation variant'],
  [orchestrationErrors, 'ProviderAfterPersistence {', 'provider-after-persistence variant'],
  [orchestrationErrors, 'PersistenceAfterProvider {', 'persistence-after-provider variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  'correlation_id = %port_context.correlation_id.as_str()',
  'actor = ?port_context.actor.id',
  'tenant_id = %context.tenant_id.to_string()',
  'fulfillment_id = ?context.fulfillment_id.map',
  'order_id = ?context.order_id.map',
  'error.to_string()',
  'format!("Fulfillment request is invalid:',
  'HttpError::bad_request("commerce_admin_fulfillment_invalid", error',
]) forbidText(source, value, 'raw or public-leaking admin fulfillment mapping');

if (failures.length > 0) {
  console.error('Commerce admin fulfillment diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin fulfillment mappers preserve typed policy while emitting bounded diagnostic shapes',
);
