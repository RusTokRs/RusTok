#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const dto = read('crates/rustok-payment/src/dto/payment.rs');
const service = read('crates/rustok-payment/src/services/payment.rs');
const refundCreation = read('crates/rustok-payment/src/services/refund_creation.rs');
const execution = read('crates/rustok-payment/src/checkout_execution.rs');
const compensation = read('crates/rustok-payment/src/checkout_compensation.rs');
const orchestration = read('crates/rustok-commerce/src/services/payment_orchestration.rs');
const webhookLifecycle = read(
  'crates/rustok-payment/src/services/provider_event_lifecycle.rs',
);
const fulfillmentStage = read(
  'crates/rustok-commerce/src/services/checkout_fulfillment_stages.rs',
);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['pub enum PaymentCollectionStatusKind', 'collection status enum'],
  ['pub enum PaymentStatusKind', 'payment status enum'],
  ['pub enum RefundStatusKind', 'refund status enum'],
  ['pub fn status_kind(&self) -> PaymentCollectionStatusKind', 'collection typed accessor'],
  ['pub fn status_kind(&self) -> PaymentStatusKind', 'payment typed accessor'],
  ['pub fn status_kind(&self) -> RefundStatusKind', 'refund typed accessor'],
  ['pub const fn as_str(self) -> Option<&\'static str>', 'canonical storage names'],
  ['pub const fn can_authorize(self) -> bool', 'authorize predicate'],
  ['pub const fn can_capture(self) -> bool', 'capture predicate'],
  ['pub const fn can_cancel(self) -> bool', 'cancel predicate'],
  ['pub const fn can_complete(self) -> bool', 'refund completion predicate'],
  ['_ => Self::Unknown', 'unknown fail-closed mapping'],
]) {
  requireText(dto, value, label);
}

for (const [value, label] of [
  ['PaymentCollectionStatusKind::from_raw(collection.status.as_str()).can_authorize()', 'owner authorize policy'],
  ['PaymentCollectionStatusKind::from_raw(collection.status.as_str()).can_capture()', 'owner capture policy'],
  ['let collection_status = PaymentCollectionStatusKind::from_raw(collection.status.as_str())', 'owner cancel policy'],
  ['RefundStatusKind::from_raw(refund.status.as_str()).can_complete()', 'refund completion policy'],
  ['RefundStatusKind::from_raw(refund.status.as_str()).can_cancel()', 'refund cancel policy'],
  ['RefundStatusKind::from_raw(refund.status.as_str()) == RefundStatusKind::Refunded', 'refunded aggregation'],
  ['normalize_collection_status_filter', 'typed collection list filter'],
  ['normalize_refund_status_filter', 'typed refund list filter'],
]) {
  requireText(service, value, label);
}

for (const [value, label] of [
  ['PaymentCollectionStatusKind::from_raw(collection.status.as_str())', 'refund collection admission'],
  ['RefundStatusKind::from_raw(existing.status.as_str()) == RefundStatusKind::Unknown', 'unknown refund replay'],
]) {
  requireText(refundCreation, value, label);
}

for (const [value, label] of [
  ['PaymentCollectionStatusKind', 'execution typed status import'],
  ['match collection.status_kind()', 'execution typed dispatch'],
  ['PaymentCollectionStatusKind::Cancelled', 'execution cancelled classification'],
  ['PaymentCollectionStatusKind::Unknown', 'execution unknown reconciliation'],
  ['payment collection lifecycle is unknown before authorization', 'authorize unknown outcome'],
  ['payment collection lifecycle is unknown before capture', 'capture unknown outcome'],
]) {
  requireText(execution, value, label);
}

for (const [value, label] of [
  ['PaymentCollectionStatusKind', 'compensation typed status import'],
  ['match collection.status_kind()', 'compensation typed dispatch'],
  ['current.status_kind() == PaymentCollectionStatusKind::Cancelled', 'cancel race adoption'],
  ['collection.status_kind() == PaymentCollectionStatusKind::Authorized', 'provider cancel predicate'],
  ['PaymentCollectionStatusKind::Unknown', 'unknown compensation reconciliation'],
]) {
  requireText(compensation, value, label);
}

for (const [value, label] of [
  ['match collection.status_kind()', 'provider orchestration typed dispatch'],
  ['collection.status_kind() != PaymentCollectionStatusKind::Captured', 'typed refund provider preflight'],
  ['collection.status_kind() == PaymentCollectionStatusKind::Authorized', 'typed provider cancel decision'],
]) {
  requireText(orchestration, value, label);
}

for (const [value, label] of [
  ['match collection.status_kind()', 'webhook typed lifecycle dispatch'],
  ['PaymentCollectionStatusKind::Unknown', 'webhook unknown classification'],
  ['payment.webhook_capture_out_of_order', 'webhook typed out-of-order capture'],
]) {
  requireText(webhookLifecycle, value, label);
}

requireText(
  fulfillmentStage,
  'state.payment_collection.status_kind() != PaymentCollectionStatusKind::Captured',
  'mounted fulfillment typed captured-state admission',
);

for (const value of [
  'refund.status != STATUS_REFUND_PENDING',
  'collection.status != STATUS_PENDING',
  'collection.status != STATUS_AUTHORIZED',
  'collection.status == STATUS_CAPTURED',
  'collection.status == STATUS_CANCELLED',
  'refund.status == STATUS_REFUNDED',
]) {
  forbidText(service, value, 'payment owner raw lifecycle policy');
}

for (const source of [execution, compensation, orchestration, webhookLifecycle]) {
  for (const value of [
    'match collection.status.as_str()',
    'current.status == "cancelled"',
    'collection.status == "authorized"',
    'collection.status == "captured"',
  ]) {
    forbidText(source, value, 'payment consumer raw collection lifecycle');
  }
}

forbidText(
  refundCreation,
  'collection.status != "captured"',
  'refund creation raw collection lifecycle',
);
forbidText(
  fulfillmentStage,
  'state.payment_collection.status != "captured"',
  'mounted fulfillment raw captured-state admission',
);

if (failures.length > 0) {
  console.error('Payment typed lifecycle status verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Payment owner, checkout/provider orchestration, webhook application, and mounted recovery use canonical typed lifecycle policy',
);
