#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';

const script = path.resolve('scripts/verify/run-index-partition-evidence.mjs');

const fixture = () => {
  const workspace = mkdtempSync(path.join(os.tmpdir(), 'index-partition-capture-plan-'));
  const manifest = path.join(workspace, 'manifest.json');
  const queryAudit = path.join(workspace, 'query-audit.json');
  const root = path.join(workspace, 'bundle');
  writeFileSync(manifest, '{}\n');
  writeFileSync(queryAudit, '{}\n');
  return { workspace, manifest, queryAudit, root };
};

const runPlan = ({ workspace, manifest, queryAudit, root }) => spawnSync(
  process.execPath,
  [
    script,
    '--plan',
    '--manifest', manifest,
    '--query-audit', queryAudit,
    '--root', root,
  ],
  {
    encoding: 'utf8',
    env: {
      ...process.env,
      DATABASE_URL: 'postgres://secret-value-must-not-be-printed',
      INDEX_PARTITION_ALLOW_FULL_CAPTURE: '1',
      CARGO: path.join(workspace, 'missing-cargo'),
    },
  },
);

test('prints a no-write eight-stage full-capture plan', () => {
  const context = fixture();
  try {
    const result = runPlan(context);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stderr, '');
    assert.equal(existsSync(context.root), false);

    const plan = JSON.parse(result.stdout);
    assert.equal(plan.contract, 'index_partition_full_capture_plan_v1');
    assert.equal(plan.mode, 'plan');
    assert.equal(plan.preflight_completed, true);
    assert.equal(plan.database_connection_attempted, false);
    assert.equal(plan.writes_performed, false);
    assert.deepEqual(plan.required_environment, [
      'INDEX_PARTITION_ALLOW_FULL_CAPTURE',
      'DATABASE_URL',
    ]);
    assert.deepEqual(
      plan.stages.map((stage) => stage.identifier),
      [
        'index-partition-snapshot-capture',
        'index-partition-query-evidence',
        'index-partition-mutation-evidence',
        'index-partition-maintenance-evidence',
        'index-partition-cutover-evidence',
        'index-partition-capture-finalize',
        'assemble-index-partition-evidence.mjs',
        'validate-index-partition-evidence.mjs',
      ],
    );
    assert.equal(Object.keys(plan.outputs.raw).length, 6);
    assert.doesNotMatch(result.stdout, /secret-value-must-not-be-printed/u);
  } finally {
    rmSync(context.workspace, { recursive: true, force: true });
  }
});

test('plan refuses partial output reuse without starting Cargo', () => {
  const context = fixture();
  try {
    mkdirSync(context.root);
    writeFileSync(path.join(context.root, 'query.json'), '{}\n');

    const result = runPlan(context);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /refusing to reuse partial partition evidence output/u);
    assert.doesNotMatch(result.stderr, /failed to start/u);
    assert.equal(existsSync(path.join(context.root, 'baseline.json')), false);
  } finally {
    rmSync(context.workspace, { recursive: true, force: true });
  }
});
