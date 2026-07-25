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

const writeJson = (filename, value) => {
  writeFileSync(filename, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
};

test('finalizer rejects a preparation placeholder hidden behind reviewed text', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-decision-placeholder-'));
  try {
    const comparisonPath = path.join(root, 'comparison.json');
    const decisionPath = path.join(root, 'decision.json');
    const outputPath = path.join(root, 'adr.md');
    writeJson(comparisonPath, {
      methodology: {
        automatic_winner_selection: false,
        comparable_database_fields: [...comparableDatabaseFields],
        database_settings_source: databaseSettingsSource,
      },
    });
    writeJson(decisionPath, {
      status: 'proposed',
      decision_date: '2026-07-25',
      owner: 'Index maintainers',
      comparison_commit: '0123456789abcdef0123456789abcdef01234567',
      comparison_sha256: '0'.repeat(64),
      selected_prototype: 'typed_eav',
      selection_rationale: 'Reviewed: TODO(index-storage-decision): explain the selected prototype.',
      rejection_rationales: {
        jsonb: 'JSONB was not selected.',
        hot_projection: 'Hot projection was not selected.',
      },
      operational_tradeoffs: 'Monitor relation growth, WAL, and VACUUM behavior.',
      migration_strategy: 'Backfill and verify parity before cutover.',
      rollback_strategy: 'Retain the previous path until cutover verification completes.',
    });

    const result = spawnSync(process.execPath, [
      finalizer,
      '--comparison', comparisonPath,
      '--decision', decisionPath,
      '--output', outputPath,
    ], { encoding: 'utf8' });

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /decision\.selection_rationale still contains a preparation placeholder/u);
    assert.equal(existsSync(outputPath), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
