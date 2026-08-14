#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const controller = readFileSync(
  new URL('crates/rustok-commerce/src/controllers/store/orders.rs', root),
  'utf8',
);
const failures = [];

const between = (start, end, label) => {
  const startIndex = controller.indexOf(start);
  const endIndex = controller.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return controller.slice(startIndex, endIndex);
};
const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const customerMapper = between(
  'fn map_storefront_customer_port_error(',
  'fn storefront_order_read_port_context(',
  'Customer read mapper',
);
const orderMapper = between(
  'fn map_storefront_order_port_error(',
  'fn map_storefront_order_command_port_error(',
  'Order read mapper',
);

for (const [content, label] of [
  [customerMapper, 'Customer read mapper'],
  [orderMapper, 'Order read mapper'],
]) {
  for (const value of [
    'correlation_id_length = context.correlation_id.chars().count()',
    'tenant_id_non_empty = !context.tenant_id.is_empty()',
    'owner_error_kind = ?error.kind',
    'owner_code_length = error.code.chars().count()',
    'retryable = error.retryable',
  ]) requireText(content, value, label);

  for (const value of [
    'error = ?error',
    'internal_message',
    'error.message',
    'correlation_id = %context.correlation_id',
    'tenant_id = %context.tenant_id',
    'actor = ?context.actor',
    'channel = ?context.channel',
    'locale = %context.locale',
  ]) forbidText(content, value, label);
}

for (const value of [
  'user_id_non_nil = !user_id.is_nil()',
  'channel_present = context.channel.is_some()',
  'locale_length = context.locale.chars().count()',
  'causation_id_present = context.causation_id.is_some()',
  'traceparent_present = context.traceparent.is_some()',
  'idempotency_key_present = context.idempotency_key.is_some()',
]) requireText(customerMapper, value, 'Customer read mapper');
for (const value of ['user_id = %user_id', 'traceparent = ?context.traceparent', 'idempotency_key = ?context.idempotency_key']) {
  forbidText(customerMapper, value, 'Customer read mapper');
}

for (const value of [
  'actor_id_non_nil = !actor_id.is_nil()',
  'customer_id_non_nil = !customer_id.is_nil()',
  'order_id_non_nil = !order_id.is_nil()',
  'channel_present = context.channel.is_some()',
  'locale_length = context.locale.chars().count()',
  'public_code = code',
  'status = %status',
]) requireText(orderMapper, value, 'Order read mapper');
for (const value of ['actor_id = %actor_id', 'customer_id = %customer_id', 'order_id = %order_id']) {
  forbidText(orderMapper, value, 'Order read mapper');
}

if (failures.length > 0) {
  console.error('Commerce storefront order read diagnostic verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ storefront customer and Order read mappers emit bounded diagnostics only');
