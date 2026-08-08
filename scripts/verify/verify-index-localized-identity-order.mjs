#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-localized-identity-order] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-index/src/domain/localized_query.rs', [
  'pub identity_order_direction: OrderDirection',
  'default_identity_order_direction',
  'identity_order_direction: OrderDirection::Asc',
  'pub fn with_identity_order_direction(',
]);
requireMarkers('crates/rustok-index/src/application/localized_validation.rs', [
  'InvalidIdentityOrderDirection',
  'OrderDirection::Asc | OrderDirection::Desc',
]);
requireMarkers('crates/rustok-index/src/application/localized_cursor.rs', [
  'identity_order_direction: OrderDirection',
  'identity_order_direction: query.identity_order_direction',
  'LOCALIZED_SCOPED_CURSOR_VERSION: u8 = 3',
]);
const compiler = requireMarkers('crates/rustok-index/src/application/postgres_localized_query.rs', [
  'identity_order_direction: OrderDirection',
  'compile_keyset(query, plan, cursor, &mut bindings)',
  'compile_order(query, plan, &mut bindings)',
  'OrderDirection::Asc => format!(',
  'OrderDirection::Desc => format!(',
  '.entity_id > {entity_id}',
  '.entity_id < {entity_id}',
  'identity_direction = match query.identity_order_direction',
]);
if (!compiler.includes('"{}.entity_id {identity_direction}"')) {
  fail('localized ORDER BY must use the explicit identity tie-break direction');
}

const ordinary = read('crates/rustok-index/src/application/postgres_query_sql.rs');
if (!ordinary.includes('entity_id ASC')) {
  fail('ordinary exact-locale query identity tie-break must remain unchanged');
}
if (ordinary.includes('identity_order_direction')) {
  fail('ordinary exact-locale compiler must not absorb localized identity ordering');
}

const owner = requireMarkers('crates/rustok-product/src/services/catalog/queries.rs', [
  '.order_by_asc(entities::product::Column::Id)',
  '.order_by_desc(entities::product::Column::Id)',
]);
if (!owner.includes('StorefrontProductSortDirection::Desc')) {
  fail('owner Storefront descending ordering contract is missing');
}

console.log('[verify-index-localized-identity-order] localized root identity tie-break can mirror owner asc/desc without changing ordinary IndexQuery');