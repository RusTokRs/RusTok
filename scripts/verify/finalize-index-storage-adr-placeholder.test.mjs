#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import {
  comparableDatabaseFields,
  databaseSettingsSource,
} from './index-storage-database-settings-contract.mjs';

const finalizer = path.resolve('scripts/verify/finalize-index-storage-adr.mjs');
const placeholderPrefix = 'TODO(index-storage-decision):';

const writeJson = (filename, value) => {
  writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
};

const comparison = () => ({
  methodology: {
    source_oracle: 'normalized idx_bench_source workload result digests',
    result_digest: 'ordered_length_prefixed_json_v1',
    evidence_validation: 'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities',
    first_run: 'first EXPLAIN ANALYZE repetition',
    warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison',
    automatic_winner_selection: false,
    comparable_database_fields: [...comparableDatabaseFields],
    database_settings_source: databaseSettingsSource,
  },
});

const decision = () => ({
  status: 'accepted',
  decision_date: '2026-07-25',
  owner: 'Index maintainers',
  comparison_commit: '0123456789abcdef0123456789abcdef01234567',
  comparison_sha256: '0'.repeat(64),
  selected_prototype: 'typed_eav',
  selection_rationale: 'Measured evidence supports the selected prototype.',
  rejection_rationales: {
    jsonb: 'JSONB was not selected.',
    hot_projection: 'Hot projection was not selected.',
  },
  operational_tradeoffs: 'Monitor relation growth, WAL, and VACUUM behavior.',
  migration_strategy: 'Backfill and verify parity before cutover.',
  rollback_strategy: 'Retain the previous path until cutover verification completes.',
});

const scenarios = [
  {
    label: 'decision.selection_rationale',
    apply: (value) => {
      value.selection_rationale = `Reviewed: ${placeholderPrefix} explain the selected prototype.`;
    },
  },
  {
    label: 'decision.operational_tradeoffs',
    apply: (value) => {
      value.operational_tradeoffs = `Reviewed operations.\n${placeholderPrefix} explain monitoring trade-offs.`;
    },
  },
  {
    label: 'decision.migration_strategy',
    apply: (value) => {
      value.migration_strategy = `Backfill first; ${placeholderPrefix} explain the final cutover.`;
    },
  },
  {
    label: 'decision.rollback_strategy',
    apply: (value) => {
      value.rollback_strategy = `Retain the old path. ${placeholderPrefix} explain rollback verification.`;
    },
  },
  {
    label: 'decision.rejection_rationales.jsonb',
    apply: (value) => {
      value.rejection_rationales.jsonb = `Reviewed JSONB. ${placeholderPrefix} explain its rejection.`;
    },
  },
  {
    label: 'decision.rejection_rationales.hot_projection',
    apply: (value) => {
      value.rejection_rationales.hot_projection = `Reviewed projection.\n${placeholderPrefix} explain its rejection.`;
    },
  },
];

for (const scenario of scenarios) {
  test(`finalizer rejects an embedded preparation placeholder in ${scenario.label}`, () => {
    const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-decision-placeholder-'));
    try {
      const comparisonPath = path.join(root, 'comparison.json');
      const decisionPath = path.join(root, 'decision.json');
      const outputPath = path.join(root, 'adr.md');
      const decisionValue = decision();
      scenario.apply(decisionValue);
      writeJson(comparisonPath, comparison());
      writeJson(decisionPath, decisionValue);

      const result = spawnSync(process.execPath, [
        finalizer,
        '--comparison', comparisonPath,
        '--decision', decisionPath,
        '--output', outputPath,
      ], { encoding: 'utf8' });

      assert.notEqual(result.status, 0);
      assert.match(
        result.stderr,
        new RegExp(`${scenario.label.replaceAll('.', '\\.') } still contains a preparation placeholder`, 'u'),
      );
      assert.equal(existsSync(outputPath), false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}
