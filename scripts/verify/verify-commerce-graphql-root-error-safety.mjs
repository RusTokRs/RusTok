#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/graphql/mod.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const value of [
  'use rustok_core::error::RichError;',
  'let rich: RichError = error.into();',
  '.user_message',
  '"Module check failed: {err}"',
  'format!("Module check failed: {err}")',
  '"Module \'{MODULE_SLUG}\' is not enabled for channel',
  'request_context.channel_slug.as_deref().unwrap_or("current")',
]) {
  forbidText(value, 'commerce GraphQL root public error mapping');
}

for (const [value, label] of [
  ['use rustok_commerce_foundation::CommerceError;', 'commerce error variant mapping'],
  ['CommerceError::Database(_)', 'database mapping'],
  ['CommerceError::ProductNotFound(_)', 'product not-found mapping'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found mapping'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate handle mapping'],
  ['CommerceError::DuplicateSku(_)', 'duplicate SKU mapping'],
  ['CommerceError::InvalidPrice(_)', 'invalid price mapping'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory mapping'],
  ['CommerceError::InvalidOptionCombination', 'option mapping'],
  ['CommerceError::Validation(_)', 'validation mapping'],
  ['CommerceError::ShippingProfileNotFound(_)', 'shipping profile mapping'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'shipping slug mapping'],
  ['CommerceError::NoVariants', 'no variants mapping'],
  ['CommerceError::CannotDeletePublished', 'published product mapping'],
  ['CommerceError::Rich(_) | CommerceError::Core(_)', 'safe fallback mapping'],
  ['"Product data is temporarily unavailable"', 'stable product storage message'],
  ['"Product was not found"', 'stable product not-found message'],
  ['"Product variant was not found"', 'stable variant not-found message'],
  [
    '"Product handle conflicts with an existing product"',
    'stable duplicate handle message',
  ],
  ['"Product request is invalid"', 'stable validation message'],
  [
    '"Product operation could not be completed safely"',
    'stable product fallback message',
  ],
  ['error = ?error', 'internal cause logging'],
  ['tenant_id = %request_context.tenant_id', 'channel tenant logging'],
  ['channel_id = ?request_context.channel_id', 'channel id logging'],
  ['channel_slug = ?request_context.channel_slug', 'channel slug logging'],
  [
    'operation = "require_storefront_channel_enabled"',
    'channel operation logging',
  ],
  [
    '"Commerce availability could not be verified"',
    'stable channel dependency message',
  ],
  [
    '"Commerce is not enabled for the current channel"',
    'stable disabled-channel message',
  ],
  ['ext.set("code", "MODULE_NOT_ENABLED")', 'stable disabled-channel code'],
]) {
  requireText(value, label);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL root error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL root errors keep technical and identifying details in structured logs and expose stable public envelopes',
);
