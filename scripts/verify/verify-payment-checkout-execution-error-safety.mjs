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

const wrapper = read('crates/rustok-payment/src/checkout_execution.rs');
const parts = [
  'types.rs',
  'diagnostic_safety.rs',
  'prepare_authorize.rs',
  'capture_provider.rs',
  'provider_helpers.rs',
  'port_impl.rs',
  'validation_identity.rs',
  'validation_errors.rs',
].map((name) =>
  read(`crates/rustok-payment/src/checkout_execution/${name}`),
);
const source = `${wrapper}\n${parts.join('\n')}`;

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const includePath of [
  'checkout_execution/types.rs',
  'checkout_execution/diagnostic_safety.rs',
  'checkout_execution/prepare_authorize.rs',
  'checkout_execution/capture_provider.rs',
  'checkout_execution/provider_helpers.rs',
  'checkout_execution/port_impl.rs',
  'checkout_execution/validation_identity.rs',
  'checkout_execution/validation_errors.rs',
]) {
  requireText(`include!("${includePath}")`, 'execution module composition');
}

for (const value of [
  '.map_err(payment_error_to_port_error)',
  'fn payment_error_to_port_error(error: PaymentError)',
  'fn manual_reconciliation(message: impl Into<String>)',
  '"PortContext.tenant_id must be a UUID for payment ports"',
  '"payment provider rejected the operation"',
  'tenant_id = %context.tenant_id',
  'internal_tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) {
  forbidText(value, 'payment checkout execution public error or raw diagnostic mapping');
}

for (const [value, label] of [
  ['const PREPARE_CHECKOUT_COLLECTION_OPERATION', 'prepare operation constant'],
  ['const AUTHORIZE_CHECKOUT_COLLECTION_OPERATION', 'authorize operation constant'],
  ['const CAPTURE_CHECKOUT_COLLECTION_OPERATION', 'capture operation constant'],
  ['const READ_CHECKOUT_COLLECTION_OPERATION', 'read operation constant'],
  ['let owner_operation = PREPARE_CHECKOUT_COLLECTION_OPERATION;', 'prepare operation mapping'],
  ['let owner_operation = AUTHORIZE_CHECKOUT_COLLECTION_OPERATION;', 'authorize operation mapping'],
  ['let owner_operation = CAPTURE_CHECKOUT_COLLECTION_OPERATION;', 'capture operation mapping'],
  ['let owner_operation = READ_CHECKOUT_COLLECTION_OPERATION;', 'read operation mapping'],
  ['correlation_id = %context.correlation_id', 'correlation logging'],
  ['tenant_id_length = context_facts.tenant_id_length', 'safe tenant shape logging'],
  ['actor_kind = context_facts.actor_kind', 'safe actor kind logging'],
  ['channel_present = context_facts.channel_present', 'safe channel presence logging'],
  ['locale_length = context_facts.locale_length', 'safe locale shape logging'],
  ['operation = owner_operation', 'owner operation logging'],
  ['code = "payment.checkout_execution_manual_reconciliation"', 'reconciliation stable code'],
  ['code = "payment.provider_request_encoding_failed"', 'request encoding stable code'],
  ['"payment request context is invalid"', 'stable context message'],
  ['"payment checkout execution requires manual reconciliation"', 'stable reconciliation message'],
  ['"payment provider rejected the requested operation"', 'stable provider rejection message'],
  ['payment_error_to_port_error(context, owner_operation, error)', 'helper context mapping'],
  ['payment_error_to_port_error(&context, owner_operation, error)', 'public context mapping'],
  ['persisted_provider_result(context, owner_operation', 'checkpoint context mapping'],
  ['parse_tenant_id(&context, owner_operation)', 'tenant context mapping'],
  ['require_operation_context(', 'causation context mapping'],
  ['checkout_payment_execution_local_operation(operation, error.code.as_str())', 'stable-code local attribution'],
]) {
  requireText(value, label);
}

if (failures.length > 0) {
  console.error('Payment checkout execution error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment checkout execution keeps technical owner/provider failures in correlation-aware safe-shape logs and exposes stable public errors',
);
