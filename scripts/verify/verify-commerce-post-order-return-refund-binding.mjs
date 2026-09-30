#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const postOrder = read(
  'crates/modules/rustok-commerce/src/services/post_order.rs',
);
const payment = read(
  'crates/modules/rustok-payment/src/services/payment.rs',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const refundStart = postOrder.indexOf(
  '    async fn create_refund_for_return(',
);
const refundEnd = postOrder.indexOf(
  '    /// Apply an exchange order change',
  refundStart,
);
if (refundStart < 0 || refundEnd < 0) {
  failures.push('post-order return refund method: unable to isolate source block');
}
const refund = refundStart >= 0 && refundEnd >= 0
  ? postOrder.slice(refundStart, refundEnd)
  : '';

for (const [value, label] of [
  ['let collection_id = match input.payment_collection_id {', 'explicit/implicit collection split'],
  ['let collection = payment_service.get_collection(tenant_id, id).await?;', 'explicit collection owner read'],
  ['validate_return_payment_collection_order(', 'explicit order-binding validation'],
  ['status: Some("captured".to_string())', 'implicit captured-only selection'],
  ['order_id: Some(order_id)', 'implicit order-scoped selection'],
  ['PaymentOrchestrationService::new(self.db.clone())', 'existing Payment execution orchestration'],
]) requireText(refund, value, label);

for (const value of [
  'Some(id) => id,',
  'let collection_id = match input.payment_collection_id {\n            Some(id) => id,',
]) forbidText(refund, value, 'raw explicit payment collection bypass');

for (const [value, label] of [
  ['fn validate_return_payment_collection_order(', 'binding helper'],
  ['collection_order_id != Some(order_id)', 'exact target-order check'],
  ['payment collection {collection_id} is not attached to order {order_id}', 'bounded validation message'],
  ['explicit_refund_collection_must_be_attached_to_target_order', 'binding regression test'],
]) requireText(postOrder, value, label);

for (const [value, label] of [
  ['pub async fn get_collection(', 'Payment collection owner read'],
  ['filter(entities::payment_collection::Column::TenantId.eq(tenant_id))', 'Payment tenant isolation'],
  ['pub async fn attach_order_to_collection(', 'collection attachment lifecycle'],
  ['existing_order_id != order_id', 'immutable conflicting order binding'],
]) requireText(payment, value, label);

if (failures.length > 0) {
  console.error('Commerce post-order return refund binding verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ legacy Commerce post-order explicit return refunds validate payment collection ownership before execution',
);
