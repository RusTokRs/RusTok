#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-collation-postgres-packet] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'fn product_title_search_condition(',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
]);
const titleSearch = owner.slice(owner.indexOf('fn product_title_search_condition('));
if (titleSearch.includes('COLLATE')) {
  fail(`${ownerPath} must remain the owner/default-collation side of the retained evidence packet`);
}

const localizedCompilerPath = 'crates/rustok-index/src/application/postgres_localized_query.rs';
const localizedCompiler = requireMarkers(localizedCompilerPath, [
  'FilterExpr::TextLike(path, pattern)',
  'IndexValueType::String => format!',
  'COLLATE \\"C\\"',
  "ESCAPE E'",
]);
if (!localizedCompiler.includes('COALESCE({} LIKE {pattern}')) {
  fail(`${localizedCompilerPath} no longer compiles TextLike through PostgreSQL LIKE`);
}

const packetPath = 'crates/rustok-distribution/tests/product_storefront_search_collation_postgres.rs';
const packet = requireMarkers(packetPath, [
  'RUSTOK_PRODUCT_STOREFRONT_COLLATION_DATABASE_URL',
  'rustok_product::migrations::migrations()',
  'IndexModule.migrations()',
  'product_storefront_default_like_matches_index_c_collation_matrix',
  'translation.title LIKE $2',
  '(translation.title COLLATE "C") LIKE $2',
  "ESCAPE E'\\\\'",
  "current_setting('lc_collate')",
  'owner_ids != index_c_ids',
  'Product Storefront title LIKE collation mismatch',
  'ASCII case-sensitive upper',
  'ASCII case-sensitive lower',
  'Unicode NFC remains byte-distinct',
  'Unicode NFD remains byte-distinct',
  'underscore wildcard',
  'escaped underscore literal',
  'percent wildcard',
  'escaped percent literal',
  'sharp-s remains distinct from ASCII SS',
  'ASCII SS remains distinct from sharp-s',
  'search: r"A\\_B"',
  'search: r"100\\%"',
]);

for (const forbidden of [
  'CREATE COLLATION',
  'ALTER TABLE product_translations',
  'SET lc_collate',
  'lower(translation.title)',
  'ILIKE',
]) {
  if (packet.includes(forbidden)) {
    fail(`${packetPath} must observe deployment/default collation rather than manufacture parity: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
]);
requireMarkers('crates/rustok-index/src/application/validation.rs', [
  'const MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024;',
]);

console.log('[verify-index-product-storefront-collation-postgres-packet] retained default-vs-C Product title LIKE matrix is source-locked; PostgreSQL execution/admission remains maintainer-owned');
