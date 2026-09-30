#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(path.resolve(configuredRoot) + path.sep)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const service = read(
  'crates/modules/rustok-commerce/src/services/shipping_profile.rs',
);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(label + ': missing ' + value);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(label + ': forbidden ' + value);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(label + ': unable to isolate source block');
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const create = between(
  service,
  'pub async fn create_shipping_profile(',
  'pub async fn list_shipping_profiles(',
  'shipping profile create',
);
const update = between(
  service,
  'pub async fn update_shipping_profile(',
  'pub async fn deactivate_shipping_profile(',
  'shipping profile update',
);

for (const [source, label] of [[create, 'create'], [update, 'update']]) {
  requireText(source, 'self.db', label + ' database handle');
  requireText(source, '.transaction::<_, _, CommerceError>', label + ' transaction wrapper');
  requireText(source, 'Box::pin(async move', label + ' async transaction body');
  requireText(source, 'await?;', label + ' transactional propagation');
}

requireText(
  service,
  'fn map_shipping_profile_transaction_error(error: TransactionError<CommerceError>) -> CommerceError',
  'explicit transaction error mapping',
);
requireText(
  service,
  'TransactionError::Connection(error) => CommerceError::Database(error)',
  'connection-level transaction error mapping',
);
requireText(
  service,
  'TransactionError::Transaction(error) => error',
  'transaction-body error preservation',
);

requireText(
  service,
  'fn is_shipping_profile_slug_conflict(error: &sea_orm::DbErr) -> bool',
  'shipping profile slug conflict classifier',
);
requireText(
  service,
  'SqlErr::UniqueConstraintViolation(details)',
  'portable unique constraint classification',
);
requireText(
  service,
  'idx_shipping_profiles_tenant_slug_unique',
  'shipping profile slug unique index identity',
);
requireText(
  service,
  'CommerceError::DuplicateShippingProfileSlug(requested_slug)',
  'create concurrent slug conflict mapping',
);
requireText(
  service,
  'CommerceError::DuplicateShippingProfileSlug(slug)',
  'update concurrent slug conflict mapping',
);

for (const [value, label] of [
  ['self.ensure_slug_available(txn,', 'create slug check inside transaction'],
  ['active_profile.insert(txn).await?', 'create profile insert inside transaction'],
  ['insert_translations(txn, id, &normalized_translations).await?', 'create translations inside transaction'],
  ['self.load_shipping_profile(txn, tenant_id, shipping_profile_id)', 'update profile load inside transaction'],
  ['active.update(txn).await?', 'update profile write inside transaction'],
  ['replace_translations(txn, shipping_profile_id, &translations).await?', 'update translations inside transaction'],
]) requireText(service, value, label);

for (const value of [
  'active_profile.insert(&self.db).await?',
  'insert_translations(&self.db, id, &normalized_translations).await?',
  'active.update(&self.db).await?',
  'replace_translations(&self.db, shipping_profile_id, &normalized_translations).await?',
]) forbidText(service, value, 'shipping profile write escaped transaction');

requireText(
  service,
  'async fn insert_translations<C: ConnectionTrait>',
  'generic transaction-compatible translation insert helper',
);
requireText(
  service,
  'async fn replace_translations<C: ConnectionTrait>',
  'generic transaction-compatible translation replacement helper',
);
requireText(
  service,
  'async fn load_shipping_profile<C: ConnectionTrait>',
  'generic transaction-compatible profile loader',
);
requireText(
  service,
  'async fn ensure_slug_available<C: ConnectionTrait>',
  'generic transaction-compatible slug check',
);

if (failures.length > 0) {
  console.error('Commerce Shipping Profile atomicity verification failed:');
  for (const failure of failures) console.error('- ' + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log('Commerce Shipping Profile create/update atomicity source guard passed');
