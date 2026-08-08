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

const productCargo = read('crates/rustok-product/Cargo.toml');
forbidMarkers('crates/rustok-product/Cargo.toml', productCargo, ['rustok-index', 'rustok-channel']);

const modulePath = 'crates/rustok-distribution/src/product_index/mod.rs';
const moduleSource = requireMarkers(modulePath, [
  'mod attribute_terms;',
  'mod channel_visibility;',
  'mod product;',
  'mod variant;',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'Lower keys are historical storage identities only.',
  'product::register(extensions)?;',
  'variant::register(extensions)?;',
  'absence::register(extensions)?;',
  'query_admission::register(extensions)?;',
  'selected_product_bridge_registers_two_current_schemas_three_factories_and_entity_admissions',
]);
forbidMarkers(modulePath, moduleSource, ['mod graph;', 'graph::', 'four_schemas']);

for (const removed of [
  'crates/rustok-distribution/src/product_index/graph.rs',
  'crates/rustok-index/docs/m7-product-source.md',
  'crates/rustok-index/docs/m7-product-variant-source.md',
  'crates/rustok-product/docs/index-graph-v3-projection-ledger.md',
  'scripts/verify/verify-index-product-graph-source.mjs',
  'scripts/verify/verify-index-product-v3-projection-ledger.mjs',
]) {
  if (fs.existsSync(resolve(removed))) fail(`removed Product compatibility artifact still exists: ${removed}`);
}

const sourcePath = 'crates/rustok-distribution/src/product_index/product.rs';
const source = requireMarkers(sourcePath, [
  'PRODUCT_INDEX_SOURCE: &str = "product-postgres-primary"',
  'PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay"',
  'derive_index_schema_source_event_id',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'fn product_schema()',
  'locale_mode: LocaleMode::Required',
  'scalar_field("seller_id", IndexValueType::String, true, false, false)?',
  'many_field("tag_ids", IndexValueType::Uuid, true, false)?',
  'scalar_field("created_at", IndexValueType::Timestamp, false, false, true)?',
  'scalar_field("published_at", IndexValueType::Timestamp, true, true, true)?',
  'many_field("attribute_terms", IndexValueType::String, false, true)?',
  'many_field("variant_ids", IndexValueType::Uuid, true, true)?',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)?',
  'name: link_name("variants")?',
  'name: link_name("sales_channels")?',
  'assert_eq!(schema.fields.len(), 15);',
  'assert_eq!(schema.links.len(), 2);',
  'product_index_graph_projection_snapshots',
  'product_sales_channel_index_relation_snapshots',
  'product_sales_channel_index_relation_freshness_snapshots',
  'channel_index_identity_generations',
  'projection.projection_epoch AS source_version',
  'product_tags product_tag',
  "COALESCE(tags.tag_ids, '[]'::jsonb) AS tag_ids",
  "COALESCE(attributes.attribute_terms, '[]'::jsonb) AS attribute_terms",
  'p.seller_id',
  'p.created_at',
  'p.published_at',
  'projection.channel_ids AS sales_channel_ids',
  'decode_product_visibility(&metadata)',
  'freshness_visibility_key != current_visibility_key',
  'freshness_channel_identity_generation < current_channel_identity_generation',
  'freshness_product_source_version > observed_product_source_version',
  'projected_product_source_version != observed_product_source_version',
  'does not require live Storefront fields or a live freshness witness',
  'FROM products p',
  'JOIN product_translations t',
  'FROM product_index_tombstones tombstone',
  'jsonb_agg(v.id ORDER BY v.id)',
  '(row.product_id, row.locale) > ($2, $3)',
  'WITH requested(product_id, locale) AS (VALUES {})',
  'IndexMutation::Delete {',
  'IndexMutation::Upsert {',
  'canonical_product_schema_contains_only_current_storefront_graph_contract',
  'canonical_product_registration_publishes_one_schema_and_one_source_factory',
  'canonical_product_sql_materializes_storefront_graph_and_eav_state',
]);
forbidMarkers(sourcePath, source, [
  'ProductSchemaVersion',
  'product_v1_schema',
  'product_v2_schema',
  'PRODUCT_EVENT_DOMAIN_V1',
  'PRODUCT_EVENT_DOMAIN_V2',
  'product-replay-v1',
  'product-replay-v2',
  'derive_index_source_event_id(',
  'SchemaVersion::new(3)',
  'FROM channels',
  'JOIN channels',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
]);

for (const currentConsumer of [
  'crates/rustok-distribution/src/product_index/absence.rs',
  'crates/rustok-distribution/src/product_index/query_admission.rs',
]) {
  const consumer = requireMarkers(currentConsumer, [
    'PRODUCT_SCHEMA_ROUTING_KEY',
    'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  ]);
  forbidMarkers(currentConsumer, consumer, ['SchemaVersion::new(3)']);
}

requireMarkers('crates/rustok-distribution/src/product_index/attribute_terms.rs', [
  'PRODUCT_ATTRIBUTE_TERMS_CTE',
  'localized_text_filter(',
  'localized_present',
]);

const graphDocPath = 'crates/rustok-index/docs/m7-product-graph-source.md';
const graphDoc = requireMarkers(graphDocPath, [
  'Status: `single_current_product_and_storefront_query_source_complete_execution_admission_pending`',
  'Current Product runtime code owns exactly one such Product key, `4`',
  'Localized identity and Storefront query boundary',
  'folds physical rows by logical identity `(tenant_id, schema_ref, entity_id)`',
  'Owner Storefront title search remains across **all Product translations**',
  'Product public projection after the fixed page',
  '`title: Null` -> `"Untitled product"`',
  '`ProductStorefrontTagReadPort`',
  'Storefront request-shape policy',
  'channel-less owner requests remain owner-native',
  'deeper owner-valid pages remain owner-native',
  'A retained PostgreSQL promotion/restart packet now exists in source',
  'Non-serving budget boundary',
  'deterministic storage-free timeout packet is retained in source',
  'The Product graph/query/Storefront source boundaries above are source-complete.',
]);
forbidMarkers(graphDocPath, graphDoc, [
  'single_current_product_source_complete_storefront_locale_query_gap_open',
  'This is now an explicit Storefront query/source gap.',
  'effective localized Storefront identity/search architecture must be resolved',
  'any generic text-pattern primitive required by that chosen architecture',
  'Storefront query translation and bounded Taxonomy tag-name hydration',
  'full owner-vs-Index Storefront equivalence',
]);

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-product-source.mjs'",
  "'verify-index-product-current-schema-promotion-postgres-packet.mjs'",
  "'verify-index-product-storefront-budgeted-execution-evidence.mjs'",
  "'verify-index-product-attribute-term-contract.mjs'",
  "'verify-index-product-channel-relation-freshness.mjs'",
  "'verify-index-linked-target-query-freshness.mjs'",
]);

console.log('[verify-index-product-source] single-current key4 Product graph and resolved Storefront query-source boundaries verified; execution/admission remains separate');
