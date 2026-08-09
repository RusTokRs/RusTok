#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-event-contract-digest-admission] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const workflowPath = '.github/workflows/event-contract-digest-admission.yml';
const workflow = requireMarkers(workflowPath, [
  'name: Event contract digest admission',
  'workflow_dispatch:',
  'default: generate_patch',
  '- verify',
  '- generate_patch',
  'permissions:',
  'contents: read',
  'persist-credentials: false',
  'toolchain: ${{ env.RUST_TOOLCHAIN }}',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
  'event-contract-digests.committed.json',
  'event-contract-digests.generated.json',
  'event-contract-digests.patch',
  'manifest.env',
  'SHA256SUMS',
  'actions/upload-artifact@v7',
  "steps.generate.outputs.status == 'drift' && inputs.mode == 'verify'",
  'no repository write was performed',
]);

for (const forbidden of [
  'contents: write',
  'pull-requests: write',
  'actions/checkout@v7\n        with:\n          persist-credentials: true',
  'git push',
  'git commit',
  'gh pr',
  'create-pull-request',
  'workflow_run:',
  'schedule:',
]) {
  if (workflow.includes(forbidden)) {
    fail(`${workflowPath} contains forbidden automatic mutation or trigger: ${forbidden}`);
  }
}

if (/^\s{2}(push|pull_request):/m.test(workflow)) {
  fail(`${workflowPath} must remain manually dispatched by contract`);
}

const generatorPath = 'crates/rustok-events/examples/event_contract_digests.rs';
requireMarkers(generatorPath, [
  'use rustok_events::event_contract_digests;',
  'format_version: 1',
  'serde_json::to_string_pretty(&artifact)',
  'contracts/event-contract-digests.json',
  'usage: cargo run -p rustok-events --example event_contract_digests [--write]',
]);

const canonicalTestPath = 'crates/rustok-events/tests/canonical_contracts.rs';
requireMarkers(canonicalTestPath, [
  'event-contract-digests.json',
  'event_contract_digests()',
  'published_event_contract_matches_committed_release_artifact',
]);

const documentationPath = 'crates/rustok-events/docs/event-contract-digest-admission.md';
requireMarkers(documentationPath, [
  'Status: `product_index_family_digest_admitted_maintainer_reverify_pending`',
  '`generate_patch`',
  '`verify`',
  '`contents: read`',
  '`persist-credentials: false`',
  '`event-contract-digests.patch`',
  'does not automatically write the repository',
  'The stale-baseline gate is complete',
  'ProductIndexRefreshEvent',
  'maintainer-provided Product-family generator output',
]);

console.log('[verify-event-contract-digest-admission] canonical digest admission workflow contract verified');
