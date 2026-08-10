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
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
  'git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json',
  'cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets',
  'cargo test --locked -p rustok-events --test canonical_contracts',
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
  '"scripts/verify/verify-index-contract-ci.mjs"',
  '".github/workflows/index-contract-ci.yml"',
]) {
  if (!workflow.includes(pathMarker)) fail(`${workflowPath} does not trigger for ${pathMarker}`);
}

console.log('[verify-index-contract-ci] focused Index contract workflow verified');
