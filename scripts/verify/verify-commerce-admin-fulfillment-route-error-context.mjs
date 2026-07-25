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

const ownerMapper = between(
  source,
  'fn map_admin_fulfillment_error(',
  'fn map_admin_fulfillment_orchestration_error(',
  'admin fulfillment owner mapper',
);
const orchestrationStart = source.indexOf('fn map_admin_fulfillment_orchestration_error(');
const orchestrationMapper =
  orchestrationStart < 0
    ? ''
    : source.slice(orchestrationStart);
if (orchestrationStart < 0) {
  failures.push('admin fulfillment orchestration mapper: unable to isolate source block');
}

for (const [value, label] of [
  ['use rustok_fulfillment::{FulfillmentError, FulfillmentService};', 'typed fulfillment import'],
  ['FulfillmentOrchestrationError, FulfillmentOrchestrationService,', 'typed orchestration import'],
  ['use rustok_web::{HttpError, HttpResult};', 'typed HTTP error import'],
  ['const ADMIN_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_routes";', 'owner constant'],
  [
    'const ADMIN_FULFILLMENT_ORCHESTRATION_OWNER: &str =',
    'orchestration owner constant',
  ],
  [
    'const ADMIN_FULFILLMENT_BOUNDARY: &str = "commerce_admin_fulfillment_http";',
    'HTTP boundary constant',
  ],
  ['struct AdminFulfillmentErrorContext {', 'route error context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['fulfillment_id: Option<Uuid>,', 'truthful fulfillment identity field'],
  ['order_id: Option<Uuid>,', 'truthful order identity field'],
  ["operation: &'static str,", 'operation context field'],
]) requireText(source, value, label);

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
  ['ListFulfillmentsInput {', 'list input contract'],
  ['page: pagination.page', 'page forwarding'],
  ['per_page: pagination.limit()', 'page-size forwarding'],
  ['let order_id = input.order_id;', 'create order identity capture'],
  ['Some(order_id)', 'create order identity context'],
  ['.create_manual_fulfillment(tenant.id, input)', 'create service contract'],
  ['.get_fulfillment(tenant.id, id)', 'show service contract'],
  ['.ship_fulfillment(tenant.id, id, input)', 'ship service contract'],
  ['.deliver_fulfillment(tenant.id, id, input)', 'deliver service contract'],
  ['.reopen_fulfillment(tenant.id, id, input)', 'reopen service contract'],
  ['.reship_fulfillment(tenant.id, id, input)', 'reship service contract'],
  ['.cancel_fulfillment(tenant.id, id, input)', 'cancel service contract'],
]) requireText(source, value, label);

for (const [value, label] of [
  ['"list_fulfillments"', 'list operation'],
  ['"create_manual_fulfillment"', 'create operation'],
  ['"get_fulfillment"', 'show operation'],
  ['"ship_fulfillment"', 'ship operation'],
  ['"deliver_fulfillment"', 'deliver operation'],
  ['"reopen_fulfillment"', 'reopen operation'],
  ['"reship_fulfillment"', 'reship operation'],
  ['"cancel_fulfillment"', 'cancel operation'],
]) requireText(source, value, label);

const ownerMapperUses =
  source.match(
    /map_admin_fulfillment_error\(\s+AdminFulfillmentErrorContext::new\(/g,
  ) ?? [];
if (ownerMapperUses.length !== 4) {
  failures.push(`expected four context-aware owner mapper callsites, found ${ownerMapperUses.length}`);
}
const orchestrationMapperUses =
  source.match(
    /map_admin_fulfillment_orchestration_error\(\s+AdminFulfillmentErrorContext::new\(/g,
  ) ?? [];
if (orchestrationMapperUses.length !== 4) {
  failures.push(
    `expected four context-aware orchestration mapper callsites, found ${orchestrationMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition variant'],
  ['FulfillmentError::Database(_)', 'database variant'],
  ['error = ?error', 'typed internal cause'],
  ['owner = ADMIN_FULFILLMENT_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['fulfillment_id = ?context.fulfillment_id', 'fulfillment identity log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_FULFILLMENT_BOUNDARY', 'boundary log'],
  ['"Fulfillment request is invalid"', 'static validation envelope'],
  ['"Commerce resource not found"', 'static not-found envelope'],
  [
    '"Fulfillment operation conflicts with the current state"',
    'static conflict envelope',
  ],
  ['"Fulfillment storage is temporarily unavailable"', 'static storage envelope'],
  ['HttpError::new(status, code, message)', 'single owner envelope constructor'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['FulfillmentOrchestrationError::Fulfillment(error)', 'nested owner delegation'],
  ['FulfillmentOrchestrationError::OrderNotFound(_)', 'order-not-found variant'],
  ['FulfillmentOrchestrationError::Database(_)', 'database variant'],
  ['FulfillmentOrchestrationError::Validation(_)', 'validation variant'],
  [
    'FulfillmentOrchestrationError::ProviderAfterPersistence {',
    'provider-after-persistence variant',
  ],
  [
    'FulfillmentOrchestrationError::PersistenceAfterProvider {',
    'persistence-after-provider variant',
  ],
  ['context.fulfillment_id = Some(*fulfillment_id);', 'persisted fulfillment identity adoption'],
  ['owner = ADMIN_FULFILLMENT_ORCHESTRATION_OWNER', 'orchestration owner log'],
  ['tenant_id = %context.tenant_id', 'orchestration tenant log'],
  ['fulfillment_id = ?context.fulfillment_id', 'orchestration fulfillment identity log'],
  ['order_id = ?context.order_id', 'orchestration order identity log'],
  ['operation = %context.operation', 'orchestration operation log'],
  ['"Fulfillment operation requires reconciliation"', 'static reconciliation envelope'],
  ['HttpError::new(status, code, message)', 'single orchestration envelope constructor'],
]) requireText(orchestrationMapper, value, label);

for (const [ownerSource, value, label] of [
  [fulfillmentErrors, 'Validation(String)', 'owner validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner database variant'],
  [orchestrationErrors, 'OrderNotFound(Uuid)', 'orchestration order-not-found variant'],
  [orchestrationErrors, 'Database(#[from] sea_orm::DbErr)', 'orchestration database variant'],
  [
    orchestrationErrors,
    'Fulfillment(#[from] rustok_fulfillment::error::FulfillmentError)',
    'orchestration fulfillment variant',
  ],
  [orchestrationErrors, 'Validation(String)', 'orchestration validation variant'],
  [orchestrationErrors, 'ProviderAfterPersistence {', 'provider-after-persistence owner variant'],
  [orchestrationErrors, 'PersistenceAfterProvider {', 'persistence-after-provider owner variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  '.map_err(super::map_fulfillment_error)',
  '.map_err(super::map_fulfillment_orchestration_error)',
  'format!("Fulfillment request is invalid:',
  'error.to_string()',
  'HttpError::bad_request("commerce_admin_fulfillment_invalid", error',
]) forbidText(source, value, 'unsafe admin fulfillment public mapping');

if (failures.length > 0) {
  console.error('Commerce admin fulfillment route error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin fulfillment routes retain typed causes and return static public envelopes',
);
