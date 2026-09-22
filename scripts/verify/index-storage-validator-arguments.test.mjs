#!/usr/bin/env node

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { runValidatorCoreWithAtomicProvenance } from './run-index-storage-validator-core.mjs';
import { validateRunnerResourceSnapshots } from './validate-index-storage-runner-resources.mjs';

const validator = path.resolve('scripts/verify/validate-index-storage-evidence.mjs');
const validatorSource = readFileSync(validator, 'utf8');
const reportFilenames = [
  'read-report.json',
  'mutation-report.json',
  'maintenance-report.json',
];

const runValidator = (args = [], env = {}) => spawnSync(process.execPath, [validator, ...args], {
  encoding: 'utf8',
  env: {
    ...process.env,
    INDEX_BENCH_SCALE: '100k',
    INDEX_BENCH_EVIDENCE_ROOT: 'missing-evidence',
    ...env,
  },
});

const expectArgumentFailure = (...args) => {
  const result = runValidator(args);
  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /validate-index-storage-evidence\.mjs does not accept arguments/u,
  );
  assert.doesNotMatch(result.stderr, /missing evidence file/u);
  assert.equal(result.stdout, '');
};

const createCoreFixture = (root, name, source) => {
  for (const filename of reportFilenames) {
    writeFileSync(path.join(root, filename), '{}\n', 'utf8');
  }
  const corePath = path.join(root, name);
  writeFileSync(corePath, source, 'utf8');
  return corePath;
};

const assertNoStagingDirectories = (root) => {
  assert.equal(
    readdirSync(root).some((entry) => entry.startsWith('.provenance-validation-')),
    false,
  );
};

const snapshotText = ({
  capturedAt,
  phase,
  runnerLabel = 'ubuntu-latest',
  nproc = 4,
  kernel = 'Linux runner 6.8.0 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux',
}) => [
  `captured_at=${capturedAt}`,
  `phase=${phase}`,
  `runner_label=${runnerLabel}`,
  kernel,
  `nproc=${nproc}`,
  '               total        used        free      shared  buff/cache   available',
  'Mem:      8000000000  2000000000  3000000000   100000000  3000000000  5000000000',
  'Swap:              0           0           0',
  'Filesystem 1B-blocks Used Available Use% Mounted on',
  '/dev/root 100000000000 10000000000 90000000000 10% /',
  '',
].join('\n');

const writeSnapshotPair = (root, overrides = {}) => {
  const before = {
    capturedAt: '2026-07-25T10:00:00Z',
    phase: 'before',
    ...overrides.before,
  };
  const after = {
    capturedAt: '2026-07-25T10:30:00Z',
    phase: 'after',
    ...overrides.after,
  };
  writeFileSync(
    path.join(root, 'runner-resources-before.txt'),
    snapshotText(before),
    'utf8',
  );
  writeFileSync(
    path.join(root, 'runner-resources-after.txt'),
    snapshotText(after),
    'utf8',
  );
};

const requiredRunnerResources = { INDEX_BENCH_REQUIRE_RUNNER_RESOURCES: '1' };

test('direct validator rejects help as an unsupported argument', () => {
  expectArgumentFailure('--help');
});

test('direct validator rejects output arguments before evidence access', () => {
  expectArgumentFailure('--output', 'ignored');
});

test('validator wires runner resource preflight before atomic core execution', () => {
  const orderingGate = validatorSource.indexOf('validatePacketReadOrdering(evidenceRoot);');
  const resourceGate = validatorSource.indexOf('validateRunnerResourceSnapshots(evidenceRoot);');
  const coreGate = validatorSource.indexOf(
    'const status = runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath });',
  );
  assert.notEqual(orderingGate, -1);
  assert.notEqual(resourceGate, -1);
  assert.notEqual(coreGate, -1);
  assert.ok(orderingGate < resourceGate);
  assert.ok(resourceGate < coreGate);
});

test('runner resource preflight is optional unless explicitly required', () => {
  assert.doesNotThrow(() => validateRunnerResourceSnapshots('missing-evidence', {}));
});

test('runner resource preflight accepts one consistent before and after pair', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-runner-resources-valid-'));
  try {
    writeSnapshotPair(root);
    assert.doesNotThrow(() => validateRunnerResourceSnapshots(root, requiredRunnerResources));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('runner resource preflight rejects an empty snapshot', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-runner-resources-empty-'));
  try {
    writeSnapshotPair(root);
    writeFileSync(path.join(root, 'runner-resources-before.txt'), '', 'utf8');
    assert.throws(
      () => validateRunnerResourceSnapshots(root, requiredRunnerResources),
      /runner-resources-before\.txt must not be empty/u,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('runner resource preflight rejects runner identity drift', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-runner-resources-drift-'));
  try {
    writeSnapshotPair(root, { after: { runnerLabel: 'different-runner' } });
    assert.throws(
      () => validateRunnerResourceSnapshots(root, requiredRunnerResources),
      /must use the same runner_label/u,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('runner resource preflight rejects reversed capture timestamps', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-runner-resources-order-'));
  try {
    writeSnapshotPair(root, {
      before: { capturedAt: '2026-07-25T11:00:00Z' },
      after: { capturedAt: '2026-07-25T10:30:00Z' },
    });
    assert.throws(
      () => validateRunnerResourceSnapshots(root, requiredRunnerResources),
      /must be ordered before then after/u,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('validator revokes stale packet provenance only when validation starts', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-validator-lifecycle-'));
  const provenancePath = path.join(root, 'provenance.json');
  try {
    writeFileSync(provenancePath, '{"packet_contract_version":2}\n', 'utf8');

    const argumentFailure = runValidator(['--help'], {
      INDEX_BENCH_EVIDENCE_ROOT: root,
    });
    assert.notEqual(argumentFailure.status, 0);
    assert.equal(existsSync(provenancePath), true);

    const validationFailure = runValidator([], {
      INDEX_BENCH_EVIDENCE_ROOT: root,
    });
    assert.notEqual(validationFailure.status, 0);
    assert.match(validationFailure.stderr, /missing evidence file: .*read-report\.json/u);
    assert.equal(existsSync(provenancePath), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('successful validator core publishes provenance from staging', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-validator-atomic-success-'));
  try {
    const corePath = createCoreFixture(root, 'success-core.mjs', `
      import { writeFileSync } from 'node:fs';
      import path from 'node:path';
      writeFileSync(path.join(process.env.INDEX_BENCH_EVIDENCE_ROOT, 'provenance.json'), '{"ok":true}\\n', 'utf8');
    `);
    const status = runValidatorCoreWithAtomicProvenance({ evidenceRoot: root, corePath });
    assert.equal(status, 0);
    assert.equal(readFileSync(path.join(root, 'provenance.json'), 'utf8'), '{"ok":true}\n');
    assertNoStagingDirectories(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('failed validator core cannot publish partial provenance', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-index-validator-atomic-failure-'));
  try {
    const corePath = createCoreFixture(root, 'failure-core.mjs', `
      import { writeFileSync } from 'node:fs';
      import path from 'node:path';
      writeFileSync(path.join(process.env.INDEX_BENCH_EVIDENCE_ROOT, 'provenance.json'), '{"partial":true}', 'utf8');
      process.exit(7);
    `);
    const status = runValidatorCoreWithAtomicProvenance({ evidenceRoot: root, corePath });
    assert.equal(status, 7);
    assert.equal(existsSync(path.join(root, 'provenance.json')), false);
    assertNoStagingDirectories(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
