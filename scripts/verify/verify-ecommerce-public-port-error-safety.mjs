#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
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

for (const [source, label] of [
  [channel, 'channel port'],
  [region, 'region port'],
  [cart, 'cart checkout port'],
]) {
  requireText(source, 'tracing::error!', label);
  forbidText(source, 'PortError::unavailable(\n                "', `${label} raw unavailable`);
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
  console.error('Ecommerce public port error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Hardened channel, region, and cart ports keep raw owner errors out of public PortError messages',
);
