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
  paymentCommands: 'crates/rustok-commerce/src/graphql_runtime/payment_commands.rs',
  collectionOwner: 'crates/rustok-payment/src/admin_collection_command.rs',
  refundOwner: 'crates/rustok-payment/src/admin_refund_command.rs',
  plan: 'crates/rustok-commerce/docs/implementation-plan.md',
  document: 'crates/rustok-commerce/docs/graphql-payment-command-owner-port-cutover-2026-08-09.md',
};

const providerOperations = read(paths.providerOperations);
const graphqlRuntime = read(paths.graphqlRuntime);
const paymentCommands = read(paths.paymentCommands);
const collectionOwner = read(paths.collectionOwner);
const refundOwner = read(paths.refundOwner);
const plan = read(paths.plan);
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  'AuthorizeAdminPaymentCollectionRequest',
  'CaptureAdminPaymentCollectionRequest',
  'CancelAdminPaymentCollectionRequest',
  'CreateAdminRefundRequest',
  'CompleteAdminRefundRequest',
  'CancelAdminRefundRequest',
  'payment_command_runtime_from_context',
  '.collection_command_port()',
  '.authorize_payment_collection(',
  '.capture_payment_collection(',
  '.cancel_payment_collection(',
  '.refund_command_port()',
  '.create_refund(',
  '.complete_refund(',
  '.cancel_refund(',
  'Permission::PAYMENTS_UPDATE',
  'PortActor::user(auth.user_id.to_string())',
  '.with_deadline(std::time::Duration::from_secs(2))',
  'request.channel_slug.as_deref()',
  'graphql-payment-collection:{collection_id}:{operation}',
  'graphql-refund:{refund_id}:{operation}',
  '.with_idempotency_key(creation_key.to_string())',
  'creation_key: idempotency_key',
]) requireText(providerOperations, marker, `${paths.providerOperations}: mounted Payment owner command contract`);

for (const forbidden of [
  'payment_orchestration_from_context',
  'PaymentOrchestrationError',
  'rustok_payment::error::PaymentError',
  'PaymentOrchestrationService',
  'PaymentProviderOperationJournal',
  'PaymentService::new',
]) forbidText(providerOperations, forbidden, `${paths.providerOperations}: concrete Payment orchestration`);

for (const marker of [
  '"payment.refund_reserved_reconciliation_required"',
  '"payment.refund_reserved_provider_unavailable"',
  '"payment.provider_invalid_response"',
  '"payment.provider_outcome_unknown"',
  '"payment.provider_unavailable"',
  '"payment.database_unavailable"',
  '"payment.provider_not_configured"',
  '"payment.provider_rejected" | "payment.invalid_transition"',
  '"PAYMENT_REQUEST_INVALID"',
  '"PAYMENT_RESOURCE_NOT_FOUND"',
  '"PAYMENT_STATE_CONFLICT"',
  '"PAYMENT_TEMPORARILY_UNAVAILABLE"',
  '"PAYMENT_RECONCILIATION_REQUIRED"',
  '"PAYMENT_CONFIGURATION_ERROR"',
  'owner_code_length = error.code.chars().count()',
  'owner_error_kind = ?error.kind',
  'boundary = "commerce_graphql_payment_command"',
]) requireText(providerOperations, marker, `${paths.providerOperations}: GraphQL Payment envelope parity`);

for (const forbidden of [
  'error = ?error',
  'owner_code = %error.code',
  'owner_message = %error.message',
  'message = %error.message',
]) forbidText(
  providerOperations.slice(
    providerOperations.indexOf('fn payment_provider_graphql_error('),
    providerOperations.indexOf('fn fulfillment_provider_graphql_error('),
  ),
  forbidden,
  `${paths.providerOperations}: bounded Payment diagnostics`,
);

for (const marker of [
  'mod payment_commands;',
  'pub use payment_commands::CommercePaymentCommandRuntime;',
  'payment_command_runtime: CommercePaymentCommandRuntime',
  'pub fn payment_command_runtime(&self) -> CommercePaymentCommandRuntime',
  '.shared_get::<CommercePaymentCommandRuntime>()',
  'CommercePaymentCommandRuntime::from_graphql_inputs(inputs)',
  'pub(crate) fn payment_command_runtime_from_context(',
  '.map(CommerceGraphqlRuntimeData::payment_command_runtime)',
  'CommercePaymentCommandRuntime::in_process(',
  'payment_provider_registry_from_context(ctx)',
]) requireText(graphqlRuntime, marker, `${paths.graphqlRuntime}: GraphQL command runtime composition`);

forbidText(
  graphqlRuntime,
  'pub(crate) fn payment_orchestration_from_context(',
  `${paths.graphqlRuntime}: mounted Payment orchestration helper`,
);

for (const marker of [
  'pub struct CommercePaymentCommandRuntime',
  'PaymentAdminCollectionCommandRuntime',
  'PaymentAdminRefundCommandRuntime',
  'pub(crate) fn from_graphql_inputs(',
  '.shared_get::<PaymentAdminCollectionCommandRuntime>()',
  '.shared_get::<PaymentAdminRefundCommandRuntime>()',
  'PaymentAdminCollectionCommandRuntime::in_process(',
  'PaymentAdminRefundCommandRuntime::in_process(',
  '.shared_get::<PaymentProviderRegistry>()',
  'pub fn collection_command_port(&self) -> Arc<dyn PaymentAdminCollectionCommandPort>',
  'pub fn refund_command_port(&self) -> Arc<dyn PaymentAdminRefundCommandPort>',
]) requireText(paymentCommands, marker, `${paths.paymentCommands}: host-selected Payment owner commands`);

for (const marker of [
  'format!("payment_collection:{}:authorize", collection.id)',
  'format!("payment_collection:{}:capture", collection.id)',
  'format!("payment_collection:{}:cancel", collection.id)',
  'PaymentProviderOperationJournal',
  'PaymentProviderRegistry',
  'PaymentService',
]) requireText(collectionOwner, marker, `${paths.collectionOwner}: durable collection provider identity`);

for (const marker of [
  '.create_or_replay(',
  'request.creation_key',
  'idempotency_key: Some(format!("payment_refund:{}", refund.id))',
  'PaymentRefundCreationService',
  'PaymentProviderOperationJournal',
  'PaymentProviderRegistry',
]) requireText(refundOwner, marker, `${paths.refundOwner}: durable refund replay identity`);

requireText(
  plan,
  '- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,\n  Payment, and Fulfillment concrete services behind host-composed owner ports.',
  `${paths.plan}: broad topology item remains open`,
);

for (const marker of [
  '# Commerce GraphQL Payment command owner-port cutover',
  'Status: `source_complete_unvalidated`',
  '`authorizePaymentCollection`',
  '`capturePaymentCollection`',
  '`cancelPaymentCollection`',
  '`createRefund`',
  '`completeRefund`',
  '`cancelRefund`',
  '`PaymentAdminCollectionCommandPort`',
  '`PaymentAdminRefundCommandPort`',
  '`PaymentRefundCreationService::create_or_replay`',
  'The broad canonical topology item remains open.',
  'No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, CI, runtime calls, provider execution, lost-response scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice.',
]) requireText(document, marker, `${paths.document}: truthful source record`);

if (failures.length > 0) {
  console.error('Commerce GraphQL Payment command owner-port cutover verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('commerce GraphQL Payment commands route through typed owner ports with preserved durable identities');
