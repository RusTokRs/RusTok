#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-maintainer-unblock-handoff] ${message}`);
  process.exit(1);
};

function requireMarkers(relative, markers) {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
}

function sectionBetween(source, startMarker, endMarker, label) {
  const start = source.indexOf(startMarker);
  if (start < 0) fail(`${label} is missing section start ${startMarker}`);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (end < 0 || end <= start) fail(`${label} is missing section end ${endMarker}`);
  return source.slice(start, end);
}

function fencedBlockAfter(source, marker, language, label) {
  const markerIndex = source.indexOf(marker);
  if (markerIndex < 0) fail(`${label} is missing block marker ${marker}`);
  const fence = `\`\`\`${language}`;
  const fenceIndex = source.indexOf(fence, markerIndex + marker.length);
  if (fenceIndex < 0) fail(`${label} is missing ${language || 'plain'} fenced block after ${marker}`);
  const bodyStart = source.indexOf('\n', fenceIndex + fence.length);
  if (bodyStart < 0) fail(`${label} has malformed fenced block after ${marker}`);
  const bodyEnd = source.indexOf('\n```', bodyStart + 1);
  if (bodyEnd < 0) fail(`${label} has unterminated fenced block after ${marker}`);
  return source.slice(bodyStart + 1, bodyEnd).trim();
}

function requireOrdered(source, markers, label) {
  let cursor = 0;
  for (const marker of markers) {
    const index = source.indexOf(marker, cursor);
    if (index < 0) fail(`${label} is missing ordered marker ${marker}`);
    cursor = index + marker.length;
  }
}

const handoffPath = 'crates/rustok-index/docs/maintainer-unblock-handoff-2026-08-09.md';
const repairPath = 'crates/rustok-index/docs/m6-repair-retained-evidence-admission.md';
const eventPath = 'crates/rustok-events/docs/event-contract-digest-admission.md';
const storefrontPath = 'crates/rustok-index/docs/m7-product-storefront-parity-gate.md';
const currentPlanPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-08.md';
const readmePath = 'crates/rustok-index/docs/README.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const handoff = requireMarkers(handoffPath, [
  'Status: `source_gates_complete_maintainer_execution_required`.',
  '## Priority 1 — M6 concrete repair PostgreSQL admission',
  '## Priority 2 — M5 canonical event-contract digest admission',
  '## Priority 3 — M7 Product Storefront evidence gates',
  '## Still blocked — partition replay',
  '## Resume rule',
  'do not infer admission from source inspection alone',
  'do not add legacy/version-family compatibility for repository-owned pre-release contracts',
  'scripts/verify/verify-index-maintainer-unblock-handoff.mjs',
]);

const repair = requireMarkers(repairPath, [
  'Status: `source_complete_owner_execution_pending`.',
  'crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json',
  'scripts/evidence/capture-index-repair-postgres.mjs',
  'scripts/verify/verify-index-repair-retained-evidence.mjs',
  'status = postgres_runtime_executed',
  'final_status = pass',
]);

const handoffExecution = fencedBlockAfter(
  handoff,
  'Run from the exact clean commit intended for admission:',
  'bash',
  handoffPath,
);
const repairExecution = fencedBlockAfter(
  repair,
  'Run from the exact clean commit intended for admission:',
  'bash',
  repairPath,
);
if (handoffExecution !== repairExecution) {
  fail('M6 handoff execution commands drifted from canonical retained-evidence admission');
}

const handoffOutputs = fencedBlockAfter(
  handoff,
  'A successful capture must produce the complete retained set together:',
  'text',
  handoffPath,
);
const repairOutputs = fencedBlockAfter(
  repair,
  'A successful run writes one atomic logical set:',
  'text',
  repairPath,
);
if (handoffOutputs !== repairOutputs) {
  fail('M6 handoff retained outputs drifted from canonical retained-evidence admission');
}

const event = requireMarkers(eventPath, [
  'Status: `source_complete_maintainer_execution_pending`.',
  'The Product Index refresh event family remains blocked until all of the following are complete:',
  '`generate_patch`',
  '`verify`',
  'ProductIndexRefreshEvent',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
]);
const eventDependency = sectionBetween(event, '## Product Index dependency', '## Deliberate limits', eventPath);
const handoffEvent = sectionBetween(
  handoff,
  '## Priority 2 — M5 canonical event-contract digest admission',
  '## Priority 3 — M7 Product Storefront evidence gates',
  handoffPath,
);
requireOrdered(eventDependency, [
  'generate_patch',
  'commit the canonical generated artifact',
  'verify',
  'ProductIndexRefreshEvent',
], `${eventPath} Product Index dependency`);
requireOrdered(handoffEvent, [
  'generate_patch',
  'event-contract-digests.json',
  'verify',
  'ProductIndexRefreshEvent',
], `${handoffPath} M5 priority`);
if (!handoffEvent.includes('stale committed event-contract digest artifact')) {
  fail('M5 handoff must continue to describe the committed event-contract digest as stale');
}

const storefront = requireMarkers(storefrontPath, [
  'Status: `budgeted_timeout_evidence_source_complete_execution_pending`.',
  'Mounted Storefront remains owner-native',
  'maintainer execution/admission is not claimed',
  'traffic switch remains last',
  'Do not move mounted Storefront traffic from source inspection alone.',
]);
const handoffStorefront = sectionBetween(
  handoff,
  '## Priority 3 — M7 Product Storefront evidence gates',
  '## Still blocked — partition replay',
  handoffPath,
);
for (const marker of [
  'Mounted Storefront must remain owner-native until retained evidence is executed/admitted.',
  'deterministic budgeted timeout evidence',
  'Product key-4 promotion/restart PostgreSQL packet',
  'current-key Storefront core/EAV/collation',
  'only then a real tenant stage/rebuild/`register_current`',
  'followed last by any eligible traffic switch',
]) {
  if (!handoffStorefront.includes(marker)) fail(`${handoffPath} M7 priority is missing ${marker}`);
}
if (!storefront.includes('channel-less requests remain typed owner-native on Product key `4`')) {
  fail(`${storefrontPath} must retain channel-less owner-native policy`);
}
if (!storefront.includes('deeper valid pages remain typed owner-native')) {
  fail(`${storefrontPath} must retain deep-page owner-native policy`);
}

const plan = requireMarkers(currentPlanPath, [
  'There is no remaining independent source-only M6 replay expansion justified by the current contract.',
  'M5 typed Product event work remains gated by canonical event-contract digest admission.',
  'M7 serving cutover remains gated by retained evidence execution/admission.',
  'Partition replay remains blocked: no real partition-capable source contract can yet filter a partition before',
  'pagination, so do not merely populate `partition_key`.',
]);
if (!plan.includes('Further source changes should follow a concrete defect discovered by executed evidence.')) {
  fail(`${currentPlanPath} must keep source continuation tied to executed evidence defects`);
}

requireMarkers(readmePath, [
  '[Maintainer Unblock Handoff — 2026-08-09](./maintainer-unblock-handoff-2026-08-09.md)',
]);
requireMarkers(aggregatePath, [
  "'verify-index-maintainer-unblock-handoff.mjs',",
]);

for (const forbidden of [
  'Status: `admitted`',
  'Status: `production_ready`',
  'ProductIndexRefreshEvent is admitted',
  'Storefront traffic is switched',
  'partition replay is source-complete',
]) {
  if (handoff.includes(forbidden)) fail(`${handoffPath} must not claim ${forbidden}`);
}

console.log('[verify-index-maintainer-unblock-handoff] handoff remains synchronized with pending M5/M6/M7 gates and fail-closed resume rules');
