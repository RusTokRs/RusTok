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

const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const ownerReadme = read('crates/modules/rustok-fulfillment/README.md');
const ownerService = read('crates/modules/rustok-fulfillment/src/services/fulfillment.rs');
const translationEntity = read(
  'crates/modules/rustok-fulfillment/src/entities/shipping_option_translation.rs',
);

const translationOwnerService = read(
  'crates/modules/rustok-fulfillment/src/services/shipping_option_translation.rs',
);

for (const marker of [
  'pub struct ShippingOptionTranslationService',
  'lock_exclusive()',
  'shipping_option_translation::Entity::update_many()',
  'rows_affected != 1',
  'insert_translation_for_tenant(',
  'INSERT INTO shipping_option_translations (id, shipping_option_id, locale, name)',
  'SELECT $1, id, $2, $3',
  'WHERE id = $4 AND tenant_id = $5',
]) {
  requireText(translationOwnerService, marker, 'exact-locale translation owner scope');
}
for (const marker of [
  'shipping_option_translation::Relation::ShippingOption.def()',
  'shipping_option::Column::TenantId.eq(tenant_id)',
  'load_translations(&txn, tenant_id, shipping_option_id)',
]) {
  requireText(translationOwnerService, marker, 'exact-locale tenant-scoped child reads');
}
for (const value of [
  'active.update(&txn).await?',
  'load_translations(&txn, shipping_option_id)',
  'load_translations(&self.db, shipping_option_id)',
]) {
  forbidText(translationOwnerService, value, 'exact-locale tenantless child mutation/read');
}

for (const marker of [
  '## Translation ownership',
  'Shipping-option translation updates and deletes remain tenant-scoped at the mutation-query layer',
  'no duplicate tenant_id is stored on the translation table',
]) requireText(ownerReadme, marker, 'Fulfillment translation ownership contract');

for (const marker of [
  'async fn synchronize_translations(',
  'tenant_id: Uuid,',
  'Entity::update_many()',
  'Entity::delete_many()',
  'shipping_option_tenant_exists(tenant_id, shipping_option_id)',
  'Expr::exists(',
  'Query::select()',
  'entities::shipping_option::Column::TenantId.eq(tenant_id)',
  'sea_orm::DbErr::RecordNotUpdated',
]) requireText(ownerService, marker, 'translation mutation topology');

for (const marker of [
  'pub struct Model',
  'pub shipping_option_id: Uuid',
  'pub locale: String',
  'pub name: String',
  'Relation::ShippingOption',
  'to = "super::shipping_option::Column::Id"',
]) requireText(translationEntity, marker, 'translation relation');

for (const value of [
  'pub tenant_id: Uuid',
  'TenantId',
]) forbidText(
  between(
    translationEntity,
    'pub struct Model',
    'pub enum Relation',
    'translation model',
  ),
  value,
  'translation entity tenant duplication',
);

const sync = between(
  ownerService,
  'async fn synchronize_translations(',
  'async fn load_shipping_option_translation_rows<C>(',
  'translation synchronization',
);

for (const [value, label] of [
  ['Entity::update_many()', 'scoped translation update query'],
  ['Entity::delete_many()', 'scoped translation delete query'],
  ['.filter(entities::shipping_option_translation::Column::Id.eq(current.id))', 'update/delete row identity'],
  ['.filter(entities::shipping_option_translation::Column::ShippingOptionId', 'parent identity filter'],
  ['shipping_option_tenant_exists(tenant_id, shipping_option_id)', 'tenant parent scope'],
  ['if update_result.rows_affected != 1', 'update cardinality guard'],
  ['if delete_result.rows_affected != 1', 'delete cardinality guard'],
  ['sea_orm::DbErr::RecordNotUpdated', 'mutation cardinality failure'],
]) requireText(sync, value, label);

for (const value of [
  'active.update(db).await?',
  'Entity::delete_by_id(current.id)',
]) forbidText(sync, value, 'tenantless child mutation');

const tenantScope = between(
  ownerService,
  'fn shipping_option_tenant_exists(',
  'async fn load_shipping_option_translation_rows<C>(',
  'shipping-option tenant scope helper',
);

for (const [value, label] of [
  ['Expr::exists(', 'parent existence predicate'],
  ['Query::select()', 'tenant parent subquery'],
  ['entities::shipping_option::Column::Id.eq(shipping_option_id)', 'parent identity predicate'],
  ['entities::shipping_option::Column::TenantId.eq(tenant_id)', 'parent tenant predicate'],
]) requireText(tenantScope, value, label);

const updateBranch = between(
  sync,
  'Some(name) => {',
  'None => {',
  'translation update branch',
);
const deleteBranch = between(
  sync,
  'None => {',
  'for (locale, name) in desired',
  'translation delete branch',
);

for (const [source, label] of [
  [updateBranch, 'translation update branch'],
  [deleteBranch, 'translation delete branch'],
]) {
  requireText(source, 'shipping_option_tenant_exists(tenant_id, shipping_option_id)', label);
  requireText(source, 'rows_affected != 1', label);
}

if (failures.length > 0) {
  console.error('Fulfillment shipping-option translation write-scope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ shipping-option translation update/delete mutations are explicitly tenant-scoped through the parent shipping-option relation',
);
