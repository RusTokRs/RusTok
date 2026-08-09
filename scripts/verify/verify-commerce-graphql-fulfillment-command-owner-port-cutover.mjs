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

const paths = {
  providerOperations: 'crates/rustok-commerce/src/graphql/mutations/provider_operations.rs',
  graphqlRuntime: 'crates/rustok-commerce/src/graphql_runtime.rs',
  fulfillmentCommands: 'crates/rustok-commerce/src/graphql_runtime/fulfillment_commands.rs',
  lifecycleOwner: 'crates/rustok-fulfillment/src/admin_command.rs',
  createOwner: 'crates/rustok-fulfillment/src/admin_create_command.rs',
  manualCreate: 'crates/rustok-commerce/src/services/admin_manual_fulfillment_orchestration.rs',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
  document: 'crates/rustok-commerce/docs/graphql-fulfillment-command-owner-port-cutover-2026-08-09.md',
};

const providerOperations = read(paths.providerOperations);
const graphqlRuntime = read(paths.graphqlRuntime);
const fulfillmentCommands = read(paths.fulfillmentCommands);
const lifecycleOwner = read(paths.lifecycleOwner);
const createOwner = read(paths.createOwner);
const manualCreate = read(paths.manualCreate);
const plan = read(paths.plan);
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sliceBetween = (source, start, end) => {
  const from = source.indexOf(start);
  const to = end ? source.indexOf(end, from + start.length) : source.length;
  if (from < 0 || to < 0) return '';
  return source.slice(from, to);
};

for (const marker of [
  'CancelAdminFulfillmentRequest',
  'DeliverAdminFulfillmentRequest',
  'ReopenAdminFulfillmentRequest',
  'ReshipAdminFulfillmentRequest',
  'ShipAdminFulfillmentRequest',
  'fulfillment_command_runtime_from_context',
  'manual_fulfillment_owner_orchestration_from_context',
  '.lifecycle_command_port()',
  '.ship_fulfillment(',
  '.deliver_fulfillment(',
  '.reopen_fulfillment(',
  '.reship_fulfillment(',
  '.cancel_fulfillment(',
  'Permission::FULFILLMENTS_CREATE',
  'Permission::FULFILLMENTS_UPDATE',
  'PortActor::user(auth.user_id.to_string())',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  'graphql-fulfillment:{fulfillment_id}:{operation}',
  'graphql-fulfillment:create:{}:{first:016x}{second:016x}',
]) requireText(providerOperations, marker, `${paths.providerOperations}: mounted Fulfillment owner command contract`);

for (const [start, end, label] of [
  ['async fn ship_fulfillment(', 'async fn deliver_fulfillment(', 'ship'],
  ['async fn deliver_fulfillment(', 'async fn reopen_fulfillment(', 'deliver'],
  ['async fn reopen_fulfillment(', 'async fn reship_fulfillment(', 'reopen'],
  ['async fn reship_fulfillment(', 'async fn cancel_fulfillment(', 'reship'],
  ['async fn cancel_fulfillment(', null, 'cancel'],
]) {
  const method = sliceBetween(providerOperations, start, end);
  requireText(method, 'fulfillment_command_runtime_from_context', `${paths.providerOperations}: ${label} owner runtime`);
  requireText(method, '.lifecycle_command_port()', `${paths.providerOperations}: ${label} owner command port`);
  forbidText(method, 'fulfillment_orchestration_from_context', `${paths.providerOperations}: ${label} concrete orchestration`);
}

const createMethod = sliceBetween(
  providerOperations,
  'async fn create_fulfillment(',
  'async fn ship_fulfillment(',
);
for (const marker of [
  'manual_fulfillment_owner_orchestration_from_context(ctx)',
  'fulfillment_create_read_context(ctx, tenant_id, order_id)',
  'fulfillment_create_command_context(ctx, tenant_id, &create_input)',
  '.create_manual_fulfillment(read_context, write_context.clone(), create_input)',
]) requireText(createMethod, marker, `${paths.providerOperations}: mounted create owner-port composition`);
requireText(
  createMethod,
  'fulfillment_orchestration_from_context(ctx, db.clone())',
  `${paths.providerOperations}: embedded compatibility fallback remains explicit`,
);

for (const marker of [
  '"order.order_not_found"',
  '"fulfillment.reconciliation_required"',
  '"fulfillment.database_unavailable" | "order.database_unavailable"',
  '"fulfillment.invalid_transition"',
  '"ORDER_RESOURCE_NOT_FOUND"',
  '"FULFILLMENT_REQUEST_INVALID"',
  '"FULFILLMENT_RESOURCE_NOT_FOUND"',
  '"FULFILLMENT_STATE_CONFLICT"',
  '"FULFILLMENT_TEMPORARILY_UNAVAILABLE"',
  '"FULFILLMENT_RECONCILIATION_REQUIRED"',
  'owner_code_length = error.code.chars().count()',
  'owner_error_kind = ?error.kind',
  'boundary = "commerce_graphql_fulfillment_command"',
]) requireText(providerOperations, marker, `${paths.providerOperations}: GraphQL Fulfillment envelope parity`);

const ownerErrorMapper = sliceBetween(
  providerOperations,
  'fn fulfillment_owner_graphql_error(',
  'fn legacy_fulfillment_provider_graphql_error(',
);
for (const forbidden of [
  'error = ?error',
  'owner_code = %error.code',
  'owner_message = %error.message',
  'message = %error.message',
]) forbidText(ownerErrorMapper, forbidden, `${paths.providerOperations}: bounded Fulfillment diagnostics`);

for (const marker of [
  'mod fulfillment_commands;',
  'pub use fulfillment_commands::CommerceFulfillmentCommandRuntime;',
  'fulfillment_command_runtime: CommerceFulfillmentCommandRuntime',
  'pub fn fulfillment_command_runtime(&self) -> CommerceFulfillmentCommandRuntime',
  '.shared_get::<CommerceFulfillmentCommandRuntime>()',
  'CommerceFulfillmentCommandRuntime::from_graphql_inputs(inputs)',
  'pub(crate) fn fulfillment_command_runtime_from_context(',
  '.map(CommerceGraphqlRuntimeData::fulfillment_command_runtime)',
  'pub(crate) fn manual_fulfillment_owner_orchestration_from_context(',
  'runtime.order_read_runtime().order_read_port()',
  '.fulfillment_lifecycle_read_runtime()',
  '.shipping_option_read_runtime()',
  'runtime.fulfillment_command_runtime().create_command_port()',
]) requireText(graphqlRuntime, marker, `${paths.graphqlRuntime}: GraphQL Fulfillment runtime composition`);

for (const marker of [
  'pub struct CommerceFulfillmentCommandRuntime',
  'FulfillmentAdminCommandRuntime',
  'FulfillmentAdminCreateCommandRuntime',
  'pub(crate) fn from_graphql_inputs(',
  '.shared_get::<FulfillmentAdminCommandRuntime>()',
  '.shared_get::<FulfillmentAdminCreateCommandRuntime>()',
  'FulfillmentAdminCommandRuntime::in_process(',
  'FulfillmentAdminCreateCommandRuntime::in_process(',
  '.shared_get::<FulfillmentProviderRegistry>()',
  'pub fn lifecycle_command_port(&self) -> Arc<dyn FulfillmentAdminCommandPort>',
  'pub fn create_command_port(&self) -> Arc<dyn FulfillmentAdminCreateCommandPort>',
]) requireText(fulfillmentCommands, marker, `${paths.fulfillmentCommands}: host-selected Fulfillment owner commands`);

for (const marker of [
  'FulfillmentProviderOperationJournal',
  'stable_operation_key(fulfillment_id, operation, &immutable_payload)',
  '"fulfillment:{fulfillment_id}:{operation}:{first:016x}{second:016x}"',
  'mark_reconciliation_required(',
  'FulfillmentProviderRegistry',
]) requireText(lifecycleOwner, marker, `${paths.lifecycleOwner}: durable lifecycle provider identity`);

for (const marker of [
  'FulfillmentProviderOperationJournal',
  'format!("fulfillment:{}:create_label", fulfillment.id)',
  'operation: "create_label".to_string()',
  'mark_reconciliation_required(',
  'FulfillmentProviderRegistry',
]) requireText(createOwner, marker, `${paths.createOwner}: durable create-label provider identity`);

for (const marker of [
  'OrderReadPort',
  'FulfillmentReadPort',
  'ShippingOptionReadPort',
  'FulfillmentAdminCreateCommandPort',
  '.create_fulfillment(',
]) requireText(manualCreate, marker, `${paths.manualCreate}: cross-owner create policy stays on typed ports`);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  `${paths.plan}: broad topology item remains open`,
);

for (const marker of [
  '# Commerce GraphQL Fulfillment command owner-port cutover',
  'Status: `source_complete_unvalidated`',
  '`createFulfillment`',
  '`shipFulfillment`',
  '`deliverFulfillment`',
  '`reopenFulfillment`',
  '`reshipFulfillment`',
  '`cancelFulfillment`',
  '`FulfillmentAdminCommandPort`',
  '`FulfillmentAdminCreateCommandPort`',
  'The broad canonical topology item remains open.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, CI, runtime calls, provider execution, lost-response scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice.',
]) requireText(document, marker, `${paths.document}: truthful source record`);

if (failures.length > 0) {
  console.error('Commerce GraphQL Fulfillment command owner-port cutover verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('commerce GraphQL Fulfillment commands route through typed owner ports with preserved durable identities');
