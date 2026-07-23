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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const channel = read('crates/rustok-channel/src/ports.rs');
const region = read('crates/rustok-region/src/ports.rs');
const cart = read('crates/rustok-cart/src/checkout_snapshot.rs');
const pricing = read('crates/rustok-pricing/src/ports.rs');
const payment = read('crates/rustok-payment/src/ports.rs');

for (const [source, label] of [
  [channel, 'channel port'],
  [region, 'region port'],
  [cart, 'cart checkout port'],
  [pricing, 'pricing port'],
  [payment, 'payment collection port'],
]) {
  requireText(source, 'tracing::error!', label);
}

for (const value of [
  'error.to_string(),\n            true',
  'error.to_string(),\n            false',
  'format!("channel port serialization failed: {error}")',
  'format!("region port failed: {error}")',
]) {
  forbidText(channel + region, value, 'channel/region public error mapping');
}

for (const value of [
  'CartError::Validation(error.to_string())',
  'format!("failed to serialize cart projection: {error}")',
  'format!("failed to serialize cart snapshot: {error}")',
  'PortError::validation("cart.checkout_validation", message)',
]) {
  forbidText(cart, value, 'cart checkout public error mapping');
}

for (const value of [
  'format!("pricing storage unavailable: {error}")',
  '"pricing.rich_error",\n            error.to_string()',
  '"pricing.core_error",\n            error.to_string()',
  'PortError::validation("pricing.validation", message)',
  '.map_err(pricing_error_to_port_error)',
]) {
  forbidText(pricing, value, 'pricing public error mapping');
}

for (const value of [
  'PortError::validation("payment.validation", message)',
  'format!("invalid payment transition from `{from}` to `{to}`")',
  'format!("payment provider `{provider_id}` is unavailable for `{operation}`")',
  'format!("payment provider `{provider_id}` rejected `{operation}`")',
  'format!("payment provider `{provider_id}` outcome is unknown for `{operation}`")',
  '.map_err(payment_error_to_port_error)',
]) {
  forbidText(payment, value, 'payment collection public error mapping');
}

for (const [source, value, label] of [
  [pricing, 'correlation_id = %context.correlation_id', 'pricing correlation logging'],
  [pricing, 'tenant_id = %context.tenant_id', 'pricing tenant logging'],
  [pricing, 'operation,', 'pricing owner operation logging'],
  [pricing, 'code = "pricing.database_unavailable"', 'pricing database stable code'],
  [pricing, 'code = "pricing.validation"', 'pricing validation stable code'],
  [pricing, 'code = "pricing.rich_error"', 'pricing rich stable code'],
  [pricing, 'code = "pricing.core_error"', 'pricing core stable code'],
  [pricing, '"pricing storage is temporarily unavailable"', 'pricing stable storage message'],
  [pricing, '"pricing operation failed an internal invariant"', 'pricing stable invariant message'],
  [pricing, '"pricing request is invalid"', 'pricing stable validation message'],
  [pricing, 'pricing_error_to_port_error(&context, "resolve_product_price"', 'pricing operation mapping'],
  [pricing, 'pricing_error_to_port_error(&context, "upsert_variant_price"', 'pricing write operation mapping'],
  [payment, 'correlation_id = %context.correlation_id', 'payment correlation logging'],
  [payment, 'tenant_id = %context.tenant_id', 'payment tenant logging'],
  [payment, 'operation = owner_operation', 'payment owner operation logging'],
  [payment, 'code = "payment.validation"', 'payment validation stable code'],
  [payment, 'code = "payment.invalid_transition"', 'payment transition stable code'],
  [payment, 'code = "payment.provider_unavailable"', 'payment unavailable stable code'],
  [payment, 'code = "payment.provider_rejected"', 'payment rejection stable code'],
  [payment, 'code = "payment.provider_invalid_response"', 'payment invalid response stable code'],
  [payment, 'code = "payment.provider_outcome_unknown"', 'payment outcome stable code'],
  [payment, 'code = "payment.provider_not_configured"', 'payment configuration stable code'],
  [payment, 'code = "payment.database_unavailable"', 'payment database stable code'],
  [payment, '"payment storage is temporarily unavailable"', 'payment stable storage message'],
  [payment, '"payment provider outcome requires reconciliation"', 'payment stable reconciliation message'],
  [payment, '"payment provider response could not be applied safely"', 'payment stable invalid response message'],
  [payment, '"payment provider rejected the requested operation"', 'payment stable rejection message'],
  [payment, '"payment request is invalid"', 'payment stable validation message'],
  [payment, 'payment_error_to_port_error(&context, "read_collection_status"', 'payment read operation mapping'],
]) {
  requireText(source, value, label);
}

requireText(
  channel,
  '"channel storage is temporarily unavailable"',
  'channel stable storage message',
);
requireText(
  region,
  '"region storage is temporarily unavailable"',
  'region stable storage message',
);
requireText(
  cart,
  '"cart checkout request or projection is invalid"',
  'cart stable validation message',
);
requireText(
  cart,
  '"cart checkout snapshot could not be encoded"',
  'cart stable encoding message',
);

if (failures.length > 0) {
  console.error('Scoped ecommerce public port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Channel, region, cart, pricing, and payment collection ports keep raw owner errors out of public PortError messages and retain correlation-safe technical logs',
);
