#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workflowPath = '.github/workflows/index-contract-ci.yml';
const workflow = fs.readFileSync(path.join(root, workflowPath), 'utf8');

const fail = (message) => {
  console.error(`[verify-index-contract-ci] ${message}`);
  process.exit(1);
};

for (const marker of [
  'name: Index Contract CI',
  'workflow_dispatch:',
  'pull_request:',
  'push:',
  'contents: read',
  'persist-credentials: false',
  'RUST_TOOLCHAIN: "1.96.0"',
  'node scripts/verify/verify-event-contract-digest-admission.mjs',
  'node scripts/verify/verify-index-product-refresh-event-family.mjs',
  'node scripts/verify/verify-index-product-refresh-delivery.mjs',
  'node scripts/verify/verify-index-product-refresh-host-consumer.mjs',
  'node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
  'git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json',
  'cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets',
  'cargo check --locked -p rustok-distribution --features mod-product --lib',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --lib',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy',
  'cargo test --locked -p rustok-events --test canonical_contracts',
  'cargo test --locked -p rustok-index source_refresh_event --lib',
  'cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib',
  'cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib',
  'name: Index Contract Gate',
]) {
  if (!workflow.includes(marker)) fail(`${workflowPath} is missing ${marker}`);
}

for (const forbidden of [
  'contents: write',
  'pull-requests: write',
  'persist-credentials: true',
  'git push',
  'git commit',
  'gh pr',
  'create-pull-request',
]) {
  if (workflow.includes(forbidden)) {
    fail(`${workflowPath} contains forbidden repository mutation: ${forbidden}`);
  }
}

for (const pathMarker of [
  '"crates/rustok-events/**"',
  '"crates/rustok-product/**"',
  '"crates/rustok-index/**"',
  '"crates/rustok-distribution/src/product_index/**"',
  '"crates/rustok-distribution/src/product_variant_index.rs"',
  '"apps/server/src/services/product_index_refresh_worker.rs"',
  '"apps/server/src/services/server_bootstrap.rs"',
  '"apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs"',
  '"scripts/verify/verify-index-product-refresh-delivery.mjs"',
  '"scripts/verify/verify-index-product-refresh-host-consumer.mjs"',
  '"scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs"',
  '"scripts/verify/verify-index-contract-ci.mjs"',
  '".github/workflows/index-contract-ci.yml"',
]) {
  if (!workflow.includes(pathMarker)) fail(`${workflowPath} does not trigger for ${pathMarker}`);
}

console.log('[verify-index-contract-ci] focused Index contract workflow verified');
