#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-graph-source] ${message}`);
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

const modulePath = 'crates/rustok-distribution/src/product_index/mod.rs';
const moduleSource = requireMarkers(modulePath, [
  'mod absence;',
  'pub(crate) mod graph;',
  'mod product;',
  'mod variant;',
  'product::register(extensions)?;',
  'variant::register(extensions)?;',
  'absence::register(extensions)',
  'selected_product_bridge_set_registers_four_schemas_and_three_stable_factories',
  '.len(),\n            4',
  'assert_eq!(factories.len(), 3);',
  'PRODUCT_ABSENCE_WATERMARK_FACTORY',
]);
forbidMarkers(modulePath, moduleSource, ['graph::register(extensions)', 'PRODUCT_GRAPH_INDEX_SOURCE']);

requireMarkers('crates/rustok-distribution/src/product_index/product.rs', [
  'pub(crate) use super::graph::PRODUCT_INDEX_SOURCE;',
  'super::graph::register_product(extensions)',
]);
requireMarkers('crates/rustok-distribution/src/product_variant_index.rs', [
  'pub(crate) use crate::product_index::graph::PRODUCT_VARIANT_INDEX_SOURCE;',
  'crate::product_index::graph::register_variant(extensions)',
]);

const sourcePath = 'crates/rustok-distribution/src/product_index/graph.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_VARIANT_INDEX_SOURCE: &str = "product-variant-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN_V1: &str = "rustok-product.product-replay-v1"',
  'PRODUCT_EVENT_DOMAIN_V2: &str = "rustok-product.product-replay-v2"',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V1: &str = "rustok-product.product-variant-replay-v1"',
  'PRODUCT_VARIANT_EVENT_DOMAIN_V2: &str = "rustok-product.product-variant-replay-v2"',
  'register_product(',
  'register_variant(',
  'product_schema_ref(1)',
  'product_schema_ref(2)',
  'product_variant_schema_ref(1)',
  'product_variant_schema_ref(2)',
  '"channel_restricted",',
  'many_field("allowed_channel_slugs", IndexValueType::String, true)?',
  'many_field("variant_ids", IndexValueType::Uuid, true)?',
  'name: link_name("variants")?',
  'target_schema: product_variant_schema_ref(2)?',
  'cardinality: LinkCardinality::Many',
  'IndexLinkValue {',
  'LinkedEntityKey {',
  'FROM products p',
  'JOIN product_translations t',
  'FROM product_variants v',
  'jsonb_agg(v.id ORDER BY v.id)',
  '(row.product_id, row.locale) > ($2, $3)',
  'ORDER BY row.product_id ASC, row.locale ASC',
  'row.variant_id > $2',
  'ORDER BY row.variant_id ASC',
  'request.limit() + 1',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'WITH requested(variant_id) AS (VALUES {})',
  'IndexValue::Boolean(!live.allowed_channel_slugs.is_empty())',
  'versioned_product_graph_preserves_v1_and_adds_product_to_variant_path',
  'versioned_sources_keep_one_stable_source_per_schema_identity',
  'channel_visibility_matches_storefront_normalization',
]);
forbidMarkers(sourcePath, source, [
  'FROM channels',
  'JOIN channels',
  'rustok_channel',
  'sales_channel',
  'ORDER BY p.index_revision',
  'ORDER BY v.index_revision',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_search',
]);

const absencePath = 'crates/rustok-distribution/src/product_index/absence.rs';
const absence = requireMarkers(absencePath, [
  'PRODUCT_ABSENCE_WATERMARK_FACTORY',
  'register_index_source_absence_provider(',
  'ProductLocaleAbsenceProvider',
  'FROM product_translations translation',
  'FROM product_index_tombstones tombstone',
]);
forbidMarkers(absencePath, absence, [
  'index_entities',
  'index_links',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
]);

const membershipPath =
  'crates/rustok-product/src/migrations/m20260731_000003_bump_product_index_revision_for_variant_membership.rs';
const membership = requireMarkers(membershipPath, [
  'rustok_product_variant_membership_bump_index_revision',
  "IF TG_OP = 'INSERT' THEN",
  "IF TG_OP = 'DELETE' THEN",
  'OLD.product_id IS NOT DISTINCT FROM NEW.product_id',
  'AFTER INSERT ON product_variants',
  'AFTER DELETE ON product_variants',
  'AFTER UPDATE OF id, tenant_id, product_id ON product_variants',
  'SET index_revision = index_revision + 1',
]);
forbidMarkers(membershipPath, membership, [
  'BEFORE UPDATE ON product_variants',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
]);
requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260731_000003_bump_product_index_revision_for_variant_membership;',
  'Box::new(m20260731_000003_bump_product_index_revision_for_variant_membership::Migration)',
]);

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index']);
const distributionCargo = read('crates/rustok-distribution/Cargo.toml');
forbidMarkers('crates/rustok-distribution/Cargo.toml', distributionCargo, [
  'rustok-product/index',
]);

requireMarkers('crates/rustok-index/docs/m7-product-graph-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-product::product@2`',
  '`rustok-product::product_variant@2`',
  '`product-postgres-primary` serves Product v1 and v2',
  '`product-variant-postgres-primary` serves ProductVariant v1 and v2',
  '`channel_restricted: boolean`',
  '`allowed_channel_slugs: many<string>`',
  'a many-cardinality `variants` link',
  'there is no Product-to-SalesChannel link yet',
  'without advancing Product `index_revision`',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-graph-source.mjs'",
]);

console.log('[verify-index-product-graph-source] OK');
