#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-eav-equivalence-postgres-packet] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const packetPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_eav_postgres_tests.rs';
const packet = requireMarkers(packetPath, [
  'RUSTOK_PRODUCT_STOREFRONT_EAV_EQUIVALENCE_DATABASE_URL',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'ProductSalesChannelRelationResolver',
  'ProductStorefrontIndexShadowExecutor',
  'materialize_postgres_index_sources',
  'materialize_postgres_index_query_runtime',
  'ProductCatalogReadRuntime::in_process',
  'weight=7',
  'label=Punainen',
  'label=Red',
  'color=red',
  'format!("color={COLOR_RED}")',
  'features=wifi',
  'color=missing',
  'color=00000000-0000-0000-0000-000000000000',
  'color=blue',
  "'label', 'text', 'product', TRUE, TRUE",
  "'color', 'select', 'product', FALSE, TRUE",
  "'features', 'multiselect', 'product', FALSE, TRUE",
  "'fi', 'Punainen'",
  "'en', 'Red'",
  "'{FEATURE_WIFI}'",
  'product_storefront_eav_postgres_retains_scalar_and_localized_term_equivalence',
  'product_storefront_eav_postgres_retains_option_code_uuid_and_never_equivalence',
]);

if (packet.includes('SchemaVersion::new(3)')) {
  fail(`${packetPath} must never use historical Product routing key 3`);
}
if (packet.includes('save_product_attribute_values(')) {
  fail(`${packetPath} is a clean materialization fixture and must not conflate owner-clock command evidence with query equivalence`);
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_shadow_eav_postgres_tests;',
  'mod storefront_shadow_postgres_tests;',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);
requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'ProductAttributeTermExpr::Never',
  'product_attribute_localized_text_expr(',
]);
requireMarkers('crates/rustok-distribution/src/product_index/storefront_shadow.rs', [
  'ProductAttributeTermExpr::Never',
  'FilterExpr::Not(Box::new(FilterExpr::IsNull(',
  'root_field("id")?',
]);

console.log('[verify-index-product-storefront-eav-equivalence-postgres-packet] current-key Product owner-vs-shadow EAV packet is source-locked for scalar/localized/option/Never semantics; execution remains a maintainer gate');
