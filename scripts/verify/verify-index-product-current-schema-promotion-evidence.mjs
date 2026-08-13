#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workflowPath = '.github/workflows/index-product-current-schema-promotion-evidence.yml';
const workflow = fs.readFileSync(path.join(root, workflowPath), 'utf8');

const fail = (message) => {
  console.error(`[verify-index-product-current-schema-promotion-evidence] ${message}`);
  process.exit(1);
};

for (const marker of [
  'name: Index Product Current Schema Promotion Evidence',
  'workflow_dispatch:',
  'pull_request:',
  'push:',
  'contents: read',
  'persist-credentials: false',
  'RUST_TOOLCHAIN: "1.96.0"',
  'RUSTOK_PRODUCT_KEY4_PROMOTION_DATABASE_URL: postgres://postgres:postgres@localhost:5432/rustok_index_product_key4_evidence',
  'image: postgres:16',
  'POSTGRES_DB: rustok_index_product_key4_evidence',
  'node scripts/verify/verify-index-product-current-schema-promotion.mjs',
  'node scripts/verify/verify-index-product-current-schema-promotion-postgres-packet.mjs',
  'node scripts/verify/verify-index-product-current-schema-promotion-evidence.mjs',
  'cargo test --locked -p rustok-distribution --features mod-product --test product_current_schema_promotion_postgres -- --nocapture',
  'workflow=${GITHUB_WORKFLOW}',
  'run_id=${GITHUB_RUN_ID}',
  'run_attempt=${GITHUB_RUN_ATTEMPT}',
  'sha=${GITHUB_SHA}',
  'event=${GITHUB_EVENT_NAME}',
  'actions/upload-artifact@v7',
  'retention-days: 90',
  'name: Product Current Schema Promotion Evidence Gate',
]) {
  if (!workflow.includes(marker)) fail(`${workflowPath} is missing ${marker}`);
}

for (const pathMarker of [
  '"crates/rustok-channel/**"',
  '"crates/rustok-product/**"',
  '"crates/rustok-index/**"',
  '"crates/rustok-distribution/src/product_index/**"',
  '"crates/rustok-distribution/tests/product_current_schema_promotion_postgres.rs"',
  '"scripts/verify/verify-index-product-current-schema-promotion.mjs"',
  '"scripts/verify/verify-index-product-current-schema-promotion-postgres-packet.mjs"',
  '"scripts/verify/verify-index-product-current-schema-promotion-evidence.mjs"',
  '".github/workflows/index-product-current-schema-promotion-evidence.yml"',
]) {
  if (!workflow.includes(pathMarker)) fail(`${workflowPath} does not trigger for ${pathMarker}`);
}

for (const forbidden of [
  'contents: write',
  'pull-requests: write',
  'persist-credentials: true',
  'git push',
  'git commit',
  'gh pr',
  'create-pull-request',
  'RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED',
]) {
  if (workflow.includes(forbidden)) {
    fail(`${workflowPath} contains forbidden mutation or unrelated production enablement: ${forbidden}`);
  }
}

if (/^\s*DATABASE_URL\s*:/m.test(workflow)) {
  fail(`${workflowPath} must use the evidence-specific Product key4 database variable, not generic DATABASE_URL`);
}

if (!workflow.includes('if: always()\n        uses: actions/upload-artifact@v7')) {
  fail(`${workflowPath} must archive bounded evidence even when the PostgreSQL packet fails`);
}

console.log('[verify-index-product-current-schema-promotion-evidence] Product key4 promotion PostgreSQL evidence workflow verified');
