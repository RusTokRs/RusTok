#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-fulfillment/src/checkout_execution_typed.rs');
const statusContract = read('crates/rustok-fulfillment/src/status.rs');
const portContract = read('crates/rustok-api/src/ports.rs');
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
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex);
};

const ensureMethod = between(
  source,
  'async fn ensure_checkout_fulfillments(',
  'async fn read_checkout_fulfillments(',
  'typed fulfillment ensure method',
);
const readMethod = between(
  source,
  'async fn read_checkout_fulfillments(',
  'pub fn in_process_checkout_fulfillment_execution_port(',
  'typed fulfillment read method',
);
const lifecycleValidator = between(
  source,
  'fn validate_checkout_fulfillment_lifecycle(',
  'fn manual_reconciliation(',
  'fulfillment lifecycle validator',
);
const reconciliationMapper = between(
  source,
  'fn manual_reconciliation(',
  '#[cfg(test)]',
  'fulfillment reconciliation mapper',
);
const tests = from(source, '#[cfg(test)]', 'fulfillment lifecycle tests');

for (const [value, label] of [
  ['use rustok_api::{PortContext, PortError};', 'typed port imports'],
  ['const MANUAL_RECONCILIATION_CODE: &str =', 'stable reconciliation code'],
  ['"fulfillment.checkout_execution_manual_reconciliation"', 'reconciliation code value'],
  ['const MANUAL_RECONCILIATION_MESSAGE: &str =', 'stable reconciliation message'],
  ['"checkout fulfillment requires manual reconciliation"', 'reconciliation message value'],
]) requireText(source, value, label);

for (const [content, operation, label] of [
  [ensureMethod, '"ensure_checkout_fulfillments"', 'ensure operation'],
  [readMethod, '"read_checkout_fulfillments"', 'read operation'],
]) {
  for (const [value, detail] of [
    ['let lifecycle_context = context.clone();', `${label} context retention`],
    ['&lifecycle_context,', `${label} validation context`],
    [operation, `${label} label`],
    ['&fulfillments,', `${label} fulfillment validation`],
  ]) requireText(content, value, detail);
}

for (const [value, label] of [
  ['context: &PortContext', 'validator context input'],
  ["operation: &'static str", 'validator operation input'],
  ['FulfillmentStatusKind::Pending', 'pending lifecycle'],
  ['FulfillmentStatusKind::Shipped', 'shipped lifecycle'],
  ['FulfillmentStatusKind::Delivered', 'delivered lifecycle'],
  ['FulfillmentStatusKind::Cancelled', 'cancelled lifecycle'],
  ['FulfillmentStatusKind::Unknown', 'unknown lifecycle'],
  ['"cancelled_after_payment_capture"', 'cancelled internal cause'],
  ['"unknown_owner_status"', 'unknown internal cause'],
  ['manual_reconciliation(', 'typed reconciliation mapper use'],
]) requireText(lifecycleValidator, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'mapper context input'],
  ["operation: &'static str", 'mapper operation input'],
  ['fulfillment: &FulfillmentResponse', 'mapper fulfillment input'],
  ["cause: &'static str", 'mapper internal cause input'],
  ['owner = "rustok_fulfillment.checkout_execution"', 'owner log'],
  ['correlation_id = %context.correlation_id', 'correlation log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['channel = ?context.channel', 'channel log'],
  ['operation,', 'operation log'],
  ['fulfillment_id = %fulfillment.id', 'fulfillment log'],
  ['order_id = %fulfillment.order_id', 'order log'],
  ['owner_status = %fulfillment.status', 'raw owner status internal log'],
  ['cause,', 'internal cause log'],
  ['code = MANUAL_RECONCILIATION_CODE', 'stable code log'],
  ['PortError::conflict(', 'typed conflict envelope'],
  ['MANUAL_RECONCILIATION_CODE,', 'public code use'],
  ['MANUAL_RECONCILIATION_MESSAGE,', 'static public message use'],
]) requireText(reconciliationMapper, value, label);

for (const value of [
  'PortError::new(',
  'checkout fulfillment is cancelled after payment capture',
  'checkout fulfillment lifecycle is unknown',
]) forbidText(source, value, 'unsafe lifecycle public envelope');

const reconciliationUses = source.match(/manual_reconciliation\(/g) ?? [];
if (reconciliationUses.length !== 3) {
  failures.push(
    `expected reconciliation mapper definition plus cancelled/unknown uses, found ${reconciliationUses.length}`,
  );
}

for (const [value, label] of [
  ['fn context() -> PortContext', 'test context fixture'],
  ['validate_checkout_fulfillment_lifecycle(', 'validator test use'],
  ['error.code, MANUAL_RECONCILIATION_CODE', 'stable code assertion'],
  ['error.message, MANUAL_RECONCILIATION_MESSAGE', 'static message assertion'],
  ['["pending", "shipped", "delivered"]', 'accepted lifecycle fixture'],
  ['["cancelled", "carrier_custom"]', 'reconciliation lifecycle fixture'],
]) requireText(tests, value, label);

for (const [content, value, label] of [
  [statusContract, 'pub enum FulfillmentStatusKind {', 'typed fulfillment status enum'],
  [statusContract, 'Pending,', 'pending status kind'],
  [statusContract, 'Shipped,', 'shipped status kind'],
  [statusContract, 'Delivered,', 'delivered status kind'],
  [statusContract, 'Cancelled,', 'cancelled status kind'],
  [statusContract, 'Unknown,', 'unknown status kind'],
  [statusContract, '_ => Self::Unknown', 'unknown status fail-close'],
  [portContract, 'pub correlation_id: String', 'port correlation field'],
  [portContract, 'pub channel: Option<String>', 'port channel field'],
  [portContract, 'pub struct PortError {', 'typed port error'],
  [portContract, 'pub fn conflict(', 'typed conflict constructor'],
]) requireText(content, value, label);

if (failures.length > 0) {
  console.error('Fulfillment checkout lifecycle error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Fulfillment checkout lifecycle failures keep owner causes internal and correlation-aware');
