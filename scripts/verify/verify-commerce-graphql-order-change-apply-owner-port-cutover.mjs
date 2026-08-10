#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const mutationsMod = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const graphql = read('crates/rustok-commerce/src/graphql/mutations/fulfillment.rs');
const graphqlRuntime = read('crates/rustok-commerce/src/graphql_runtime.rs');
const orchestration = read('crates/rustok-commerce/src/services/order_change_orchestration.rs');
const ownerRead = read('crates/rustok-order/src/order_read.rs');
const ownerCommand = read('crates/rustok-order/src/post_order_command.rs');
const plan = read('crates/rustok-commerce/docs/implementation-plan.md');
const record = read(
  'crates/rustok-commerce/docs/graphql-order-change-apply-owner-port-cutover-2026-08-10.md',
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

const applyMutation = between(
  graphql,
  'async fn apply_order_change(',
  'async fn cancel_order_change(',
  'GraphQL applyOrderChange mutation',
);
const readContext = between(
  graphql,
  'fn order_change_read_context(',
  'fn order_command_context(',
  'GraphQL order-change read context',
);
const ownerErrorMapper = between(
  graphql,
  'fn order_change_owner_graphql_error(',
  'fn post_order_graphql_error(',
  'GraphQL owner-port error mapper',
);
const applyErrorMapper = between(
  graphql,
  'fn order_change_graphql_error(',
  'fn order_change_read_context(',
  'GraphQL order-change error dispatcher',
);
const runtimeFactory = between(
  graphqlRuntime,
  'pub(crate) fn order_change_orchestration_from_context(',
  'pub(crate) fn return_completion_orchestration_from_context(',
  'GraphQL order-change runtime factory',
);
const ownerMethod = between(
  orchestration,
  'pub async fn apply_order_change_with_owner_ports(',
  '\n    }\n}',
  'mounted order-change owner-port orchestration',
);

requireText(mutationsMod, 'pub mod fulfillment;', 'mounted fulfillment mutation module');

for (const [value, label] of [
  ['[Permission::ORDERS_UPDATE]', 'orders:update admission'],
  ['current_tenant_scope(ctx, Some(tenant_id), "Apply order change")', 'tenant scope admission'],
  ['order_change_read_context(ctx, tenant_id, id)?', 'owner read context'],
  ['order_post_order_command_context(ctx, tenant_id, id, "apply_order_change")?', 'owner command context'],
  ['order_change_orchestration_from_context(ctx, db.clone(), event_bus.clone())', 'runtime orchestration factory'],
  ['.apply_order_change_with_owner_ports(', 'owner-port orchestration entrypoint'],
  ['read_context.clone()', 'read context forwarding'],
  ['command_context.clone()', 'command context forwarding'],
  ['order_change_graphql_error(', 'typed GraphQL error dispatch'],
]) requireText(applyMutation, value, label);

for (const value of [
  '.apply_order_change(tenant_id, id, difference_refund, metadata)',
  'OrderService::new(',
  '.get_order_change(',
  'match order_change.change_type.as_str()',
  '.apply_exchange_order_change(',
  '.apply_claim_order_change(',
]) forbidText(applyMutation, value, 'mounted GraphQL apply must not own concrete Order/dispatch');

for (const [value, label] of [
  ['PortActor::user(auth.user_id.to_string())', 'authenticated owner actor'],
  ['format!("commerce-graphql-order-change-read:{change_id}")', 'read correlation identity'],
  ['.with_deadline(std::time::Duration::from_secs(2))', 'bounded read deadline'],
  ['request.channel_slug.as_deref()', 'request channel forwarding'],
]) requireText(readContext, value, label);

for (const [value, label] of [
  ['owner = "rustok_order.order_change"', 'owner diagnostic label'],
  ['consumer_operation,', 'consumer operation diagnostic'],
  ['owner_operation,', 'owner operation diagnostic'],
  ['correlation_id = %context.correlation_id', 'correlation diagnostic'],
  ['owner_error_kind = ?error.kind', 'bounded owner kind'],
  ['owner_code_length = error.code.chars().count()', 'bounded owner code'],
  ['public_code = code', 'stable public code'],
  ['retryable,', 'retryability'],
  ['boundary = "commerce_graphql_order_change_owner"', 'boundary diagnostic'],
]) requireText(ownerErrorMapper, value, label);
for (const value of ['error = ?error', 'error.message', 'error.to_string()', 'internal_message']) {
  forbidText(ownerErrorMapper, value, 'GraphQL owner-port raw diagnostics');
}

for (const [value, label] of [
  ['OrderChangeOrchestrationError::OrderRead(source)', 'typed read error branch'],
  ['"read_order_change_projection"', 'read owner operation'],
  ['OrderChangeOrchestrationError::OrderCommand(source)', 'typed command error branch'],
  ['"apply_change"', 'command owner operation'],
  ['OrderChangeOrchestrationError::PostOrder(source)', 'cross-domain orchestration branch'],
  ['post_order_graphql_error(tenant_id, resource_id, "apply_order_change", source)', 'existing post-order envelope handoff'],
]) requireText(applyErrorMapper, value, label);

for (const [value, label] of [
  ['ctx.data_opt::<CommerceGraphqlRuntimeData>()', 'host runtime data lookup'],
  ['Some(runtime) => crate::OrderChangeOrchestrationService::from_order_ports(', 'host-composed service'],
  ['runtime.order_read_runtime().order_read_port()', 'host-selected Order read'],
  ['runtime.order_post_order_command_runtime().command_port()', 'host-selected Order command'],
  ['None => crate::OrderChangeOrchestrationService::new(db, event_bus)', 'embedded compatibility fallback'],
  ['with_payment_provider_registry(payment_provider_registry_from_context(ctx))', 'host payment registry preservation'],
]) requireText(runtimeFactory, value, label);

for (const [value, label] of [
  ['.read_order_change_projection(', 'owner read call'],
  ['ReadOrderChangeProjectionRequest { change_id }', 'typed owner read request'],
  ['.apply_change(', 'owner default apply call'],
  ['ApplyOrderChangeRequest {', 'typed owner apply request'],
  ['OrderChangeOrchestrationError::OrderRead', 'read error preservation'],
  ['OrderChangeOrchestrationError::OrderCommand', 'command error preservation'],
  ['.apply_exchange_order_change(', 'exchange orchestration retained'],
  ['.apply_claim_order_change(', 'claim orchestration retained'],
]) requireText(ownerMethod, value, label);
for (const value of ['OrderService::new(', '.get_order_change(']) {
  forbidText(ownerMethod, value, 'mounted owner-port orchestration concrete Order dependency');
}

for (const [source, value, label] of [
  [ownerRead, 'async fn read_order_change_projection(', 'Order read capability'],
  [ownerRead, '.get_order_change(tenant_id, request.change_id)', 'owner-local read implementation'],
  [ownerCommand, 'async fn apply_change(', 'Order apply capability'],
  [ownerCommand, '.apply_order_change(tenant_id, request.change_id, request.input)', 'owner-local apply implementation'],
]) requireText(source, value, label);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  'broad ecommerce topology P0 remains open',
);

for (const [value, label] of [
  ['# Commerce GraphQL order-change apply owner-port cutover', 'record title'],
  ['Status: `source_complete_unvalidated`', 'record status'],
  ['mounted GraphQL `applyOrderChange`', 'record scope'],
  ['CommerceOrderReadRuntime', 'record read runtime'],
  ['OrderPostOrderCommandRuntime', 'record command runtime'],
  ['admission metadata only', 'record replay limitation'],
  ['directly embedded schemas', 'record compatibility fallback'],
  ['broad topology P0 remains open', 'record open broad invariant'],
  ['No tests, Cargo commands, Node verifiers, formatter', 'record validation status'],
]) requireText(record, value, label);

if (failures.length > 0) {
  console.error('Commerce GraphQL order-change apply owner-port verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ mounted GraphQL applyOrderChange uses host-selected Order owner ports');
