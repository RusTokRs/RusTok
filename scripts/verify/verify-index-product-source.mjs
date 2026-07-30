#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-source] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const factoryPath = 'crates/rustok-index/src/infrastructure/postgres/source_factory.rs';
const factory = requireMarkers(factoryPath, [
  'pub trait PostgresIndexSourceFactory',
  'pub struct PostgresIndexSourceFactoryCatalog',
  'pub fn materialize_postgres_index_sources',
  'let mut staged = extensions.clone();',
  '*extensions = staged;',
]);
forbidMarkers(factoryPath, factory, ['rustok_product', 'tokio::spawn', 'loop {']);

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index']);
const productRoot = requireMarkers('crates/rustok-product/src/lib.rs', [
  'pub struct ProductRuntimeSelected;',
  'extensions.insert(ProductRuntimeSelected);',
  '&["taxonomy"]',
]);
forbidMarkers('crates/rustok-product/src/lib.rs', productRoot, [
  'rustok_index',
  'register_index_schema_source',
]);

const wrapper = requireMarkers('crates/rustok-distribution/src/product_index/product.rs', [
  'pub(crate) use super::graph::PRODUCT_INDEX_SOURCE;',
  'super::graph::register_product(extensions)',
]);
forbidMarkers('crates/rustok-distribution/src/product_index/product.rs', wrapper, [
  'DatabaseConnection',
  'FROM products',
]);

const sourcePath = 'crates/rustok-distribution/src/product_index/graph.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN_V1: &str = "rustok-product.product-replay-v1"',
  'fn product_v1_schema()',
  'reference: product_schema_ref(1)?',
  'locale_mode: LocaleMode::Required',
  'scalar_field("status", IndexValueType::String, false, true, true)?',
  'scalar_field("title", IndexValueType::String, false, true, true)?',
  'scalar_field("handle", IndexValueType::String, false, true, true)?',
  'scalar_field("description", IndexValueType::String, true, false, false)?',
  'scalar_field("vendor", IndexValueType::String, true, true, true)?',
  'scalar_field("product_type", IndexValueType::String, true, true, true)?',
  'links: Vec::new()',
  'product_schema_ref(1).map_err(|error| error.to_string())?',
  'product_schema_ref(2).map_err(|error| error.to_string())?',
  'ProductPostgresIndexSource { db }',
  'impl IndexSource for ProductPostgresIndexSource',
  'FROM products p',
  'JOIN product_translations t',
  '(p.id, t.locale) > ($2, $3)',
  'ORDER BY p.id ASC, t.locale ASC',
  'request.limit() + 1',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'PRODUCT_EVENT_DOMAIN_V1',
  'schema: product_schema_ref(schema_version)?',
  '#[serde(deny_unknown_fields)]',
  'versioned_product_graph_preserves_v1_and_adds_product_to_variant_path',
]);
forbidMarkers(sourcePath, source, [
  'ORDER BY p.index_revision',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_search',
]);

const migrationPath =
  'crates/rustok-product/src/migrations/m20260730_000001_add_product_index_revision.rs';
requireMarkers(migrationPath, [
  'ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1',
  'NEW.index_revision := OLD.index_revision + 1;',
  'trg_products_bump_index_revision',
  'trg_product_translations_bump_index_revision',
  'AFTER INSERT OR UPDATE OR DELETE ON product_translations',
]);

requireMarkers('crates/rustok-index/docs/m7-product-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product@1`',
  'stable `(product_id, locale)` identity',
  'Product hard deletes do not yet emit durable Index tombstones.',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-source.mjs'",
]);

console.log('[verify-index-product-source] OK');
