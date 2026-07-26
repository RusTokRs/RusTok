#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-placeholder-contract] ${message}`);
  process.exit(1);
};

const finalizer = read('scripts/verify/finalize-index-storage-adr.mjs');
const fixture = read('scripts/verify/finalize-index-storage-adr-placeholder.test.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const guide = read('crates/rustok-index/docs/storage-decision.md');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

requireMarkers(finalizer, 'ADR finalizer', [
  "const placeholderPrefix = 'TODO(index-storage-decision):'",
  'if (value.includes(placeholderPrefix))',
  "'selection_rationale'",
  "'operational_tradeoffs'",
  "'migration_strategy'",
  "'rollback_strategy'",
  'requireDecisionText(decision[field], `decision.${field}`);',
  'requireDecisionText(rationale, `decision.rejection_rationales.${prototype}`);',
  'still contains a preparation placeholder',
]);
if (finalizer.includes('startsWith(placeholderPrefix)')) {
  fail('ADR finalizer restored prefix-only placeholder detection');
}

requireMarkers(fixture, 'placeholder fixture', [
  "status: 'accepted'",
  "source_oracle: 'normalized idx_bench_source workload result digests'",
  "result_digest: 'ordered_length_prefixed_json_v1'",
  "evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities'",
  "first_run: 'first EXPLAIN ANALYZE repetition'",
  "warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison'",
  'automatic_winner_selection: false',
  'comparable_database_fields: [...comparableDatabaseFields]',
  'database_settings_source: databaseSettingsSource',
  "label: 'decision.selection_rationale'",
  "label: 'decision.operational_tradeoffs'",
  "label: 'decision.migration_strategy'",
  "label: 'decision.rollback_strategy'",
  "label: 'decision.rejection_rationales.jsonb'",
  "label: 'decision.rejection_rationales.hot_projection'",
  'still contains a preparation placeholder',
  'assert.equal(existsSync(outputPath), false)',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-placeholder-contract.mjs'",
  "scriptPath('finalize-index-storage-adr-placeholder.test.mjs')",
]);

requireMarkers(guide, 'storage decision guide', [
  'The finalizer rejects the exact marker at any position inside selection, rejection, operations, migration, or rollback rationale text.',
  '- no preparation placeholder remains anywhere in required rationale text;',
]);

console.log('[verify-index-storage-placeholder-contract] embedded decision placeholders are rejected across every required rationale field');
