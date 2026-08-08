#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-postgres-key4-fixtures] ${message}`);
  process.exit(1);
};

const productFixtures = [
  'crates/rustok-distribution/tests/product_locale_absence_postgres.rs',
  'crates/rustok-distribution/tests/product_materialized_query_freshness_postgres.rs',
  'crates/rustok-distribution/tests/product_channel_convergence_postgres.rs',
  'crates/rustok-distribution/tests/product_channel_identity_transitions_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_availability_equivalence_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_replay_redelivery_postgres.rs',
];

for (const relative of productFixtures) {
  const source = read(relative);
  if (!source.includes('SchemaVersion::new(4)')) {
    fail(`${relative} does not target current Product routing key 4`);
  }
  if (source.includes('SchemaVersion::new(3)')) {
    fail(`${relative} restored historical Product routing key 3`);
  }
  if (/entity_name\s*=\s*'product'[\s\S]{0,160}schema_version\s*=\s*3/.test(source)) {
    fail(`${relative} contains a direct historical Product schema_version = 3 assertion`);
  }
}

const productSource = read('crates/rustok-distribution/src/product_index/mod.rs');
if (!productSource.includes('PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4')) {
  fail('current Product routing key is not 4');
}
const productBridge = read('crates/rustok-distribution/src/product_index/product.rs');
for (const marker of [
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'assert_eq!(schema.fields.len(), 15);',
  'many_field("attribute_terms", IndexValueType::String, false, true)',
  'many_field("variant_ids", IndexValueType::Uuid, true, true)',
  'many_field("sales_channel_ids", IndexValueType::Uuid, true, true)',
]) {
  if (!productBridge.includes(marker)) fail(`current Product bridge is missing ${marker}`);
}
if (productBridge.includes('SchemaVersion::new(3)')) {
  fail('current Product bridge restored a historical key-3 compatibility path');
}

for (const relative of [
  'crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_availability_equivalence_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_replay_redelivery_postgres.rs',
]) {
  const source = read(relative);
  if (!source.includes('SchemaVersion::new(2)')) {
    fail(`${relative} changed the ProductVariant routing key unexpectedly`);
  }
}

for (const relative of [
  'crates/rustok-distribution/tests/product_linked_target_recreate_postgres.rs',
  'crates/rustok-distribution/tests/product_linked_target_availability_equivalence_postgres.rs',
]) {
  const source = read(relative);
  if (!source.includes('SchemaVersion::INITIAL')) {
    fail(`${relative} changed the SalesChannel routing key unexpectedly`);
  }
}

console.log('[verify-index-product-postgres-key4-fixtures] retained Product PostgreSQL fixtures target current key 4 without a key-3 compatibility path');
