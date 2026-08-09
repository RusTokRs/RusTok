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

const handoffPath = 'crates/rustok-index/docs/maintainer-unblock-handoff-2026-08-09.md';
const repairPath = 'crates/rustok-index/docs/m6-repair-retained-evidence-admission.md';
const eventPath = 'crates/rustok-events/docs/event-contract-digest-admission.md';
const familyPath = 'crates/rustok-product/docs/index-refresh-event-family.md';
const storefrontPath = 'crates/rustok-index/docs/m7-product-storefront-parity-gate.md';
const currentPlanPath = 'crates/rustok-index/docs/implementation-plan-current-2026-08-09.md';
const readmePath = 'crates/rustok-index/docs/README.md';
const aggregatePath = 'scripts/verify/verify-index-query-contract.mjs';

const handoff = requireMarkers(handoffPath, [
  'Status: `m5_baseline_verified_product_family_digest_pending_m6_m7_execution_required`.',
  '## Priority 1 — M6 concrete repair PostgreSQL admission',
  '## Priority 2 — M5 Product Index typed event family',
  '## Priority 3 — M7 Product Storefront evidence gates',
  '## Still blocked — partition replay',
  '## Resume rule',
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
  'Status: `baseline_admitted_verified_product_index_family_digest_pending`.',
  'PR #3390',
  '7983092f96e14c002c57451709de936e40c01356',
  'the digest diff was empty',
  'add `ProductIndexRefreshEvent`',
  'same reviewed',
  'wire-contract PR',
  'cargo run --locked -p rustok-events --example event_contract_digests -- --write',
]);
const family = requireMarkers(familyPath, [
  'Status: `source_ready_digest_regeneration_pending`.',
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
  'id = correlation_id = refresh_id',
  'causation_id = root_event_id',
]);
const handoffEvent = sectionBetween(
  handoff,
  '## Priority 2 — M5 Product Index typed event family',
  '## Priority 3 — M7 Product Storefront evidence gates',
  handoffPath,
);
for (const marker of [
  'PR #3390',
  'the digest diff was empty',
  'ProductIndexRefreshEvent',
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
  'id = correlation_id = refresh_id',
  'causation_id = root_event_id',
  'same wire-contract PR before merge',
]) {
  if (!handoffEvent.includes(marker)) fail(`${handoffPath} M5 priority is missing ${marker}`);
}
if (!event.includes('No GitHub Actions verification packet is claimed or fabricated.')) {
  fail(`${eventPath} must distinguish maintainer local verification from a retained workflow packet`);
}
if (!family.includes('No new loop, scheduler, retry owner, broker consumer, acknowledgement path or Index mutation route is introduced.')) {
  fail(`${familyPath} must retain the source-only family boundary`);
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
  'Status: `m5_product_refresh_family_source_ready_digest_regeneration_pending`.',
  'The stale event-contract baseline gate is complete.',
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
  'Before this source PR may merge',
  'M6 source remains complete and execution/admission-gated.',
  'M7 remains evidence/admission-gated.',
  'Partition replay remains blocked until a real source contract filters the requested partition before pagination.',
]);
if (!plan.includes('Do not add another M6 source slice unless that execution exposes a concrete source failure.')) {
  fail(`${currentPlanPath} must keep M6 source continuation tied to executed evidence defects`);
}

requireMarkers(readmePath, [
  '[Maintainer Unblock Handoff — 2026-08-09](./maintainer-unblock-handoff-2026-08-09.md)',
]);
requireMarkers(aggregatePath, [
  "'verify-index-maintainer-unblock-handoff.mjs',",
]);

for (const forbidden of [
  'Status: `production_ready`',
  'ProductIndexRefreshEvent is admitted',
  'Storefront traffic is switched',
  'partition replay is source-complete',
]) {
  if (handoff.includes(forbidden)) fail(`${handoffPath} must not claim ${forbidden}`);
}

console.log('[verify-index-maintainer-unblock-handoff] M5 baseline verified; Product family digest pending; M6/M7 execution gates retained');
