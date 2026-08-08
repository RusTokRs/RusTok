#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-equivalence-postgres-packet] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const packetPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs';
const packet = requireMarkers(packetPath, [
  'RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL',
  'ProductStorefrontIndexShadowExecutor',
  'ProductSalesChannelRelationResolver',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'materialize_postgres_index_sources',
  'materialize_postgres_index_query_runtime',
  'PostgresSchemaRegistrationStore',
  'MutationDelivery::from_event(PRODUCT_SOURCE, mutation)',
  'ProductCatalogReadRuntime::in_process',
  'Some("online".to_owned())',
  'Some(CHANNEL_ID)',
  'Some("Needle%")',
  'Some("Needle_FR")',
  'Some(r"Needle\\_FR")',
  'StorefrontProductSortDirection::Desc',
  'StorefrontProductSortDirection::Asc',
  'vec![PRODUCT_C, PRODUCT_B, PRODUCT_A]',
  'vec![PRODUCT_A, PRODUCT_B]',
  'assert_eq!(execution.authoritative.total, 3);',
  'assert_eq!(projected.exact_count, Some(3));',
  'assert_eq!(owner_a.title, "Requested A");',
  'assert_eq!(projected_string(index_a, "title")?, Some("Requested A"));',
  'assert_eq!(owner_b.title, "NeedleXFR");',
  'assert_eq!(projected_string(index_b, "title")?, Some("NeedleXFR"));',
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
  'product_storefront_localized_postgres_retains_owner_shadow_identity_and_projection_evidence',
  'product_storefront_localized_postgres_retains_wildcard_and_equal_timestamp_paging_evidence',
]);

if (packet.includes('SchemaVersion::new(3)')) {
  fail(`${packetPath} must never use historical Product routing key 3`);
}
if (packet.includes('register_current') || packet.includes('Storefront traffic')) {
  fail(`${packetPath} is evidence only and must not perform Product schema promotion or consumer cutover`);
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  '#[cfg(test)]',
  'mod storefront_shadow_postgres_tests;',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'placeholder',
  'routing key `4`',
]);

console.log('[verify-index-product-storefront-equivalence-postgres-packet] current-key Product owner-vs-shadow core PostgreSQL packet is source-locked; execution and EAV evidence remain maintainer gates');
