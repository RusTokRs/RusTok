#!/usr/bin/env node

import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const snapshotLimitBytes = 1024 * 1024;
const snapshotFilenames = Object.freeze({
  before: 'runner-resources-before.txt',
  after: 'runner-resources-after.txt',
});

const requireSingletonValue = (lines, key, label) => {
  const prefix = `${key}=`;
  const matches = lines.filter((line) => line.startsWith(prefix));
  if (matches.length !== 1) {
    throw new Error(`${label} must contain exactly one ${key} entry`);
  }
  const value = matches[0].slice(prefix.length);
  if (value.length === 0 || value !== value.trim()) {
    throw new Error(`${label} ${key} must be a non-empty trimmed value`);
  }
  return value;
};

const requireUtcTimestamp = (value, label) => {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(value)) {
    throw new Error(`${label} must be a real UTC timestamp`);
  }
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.valueOf())
      || parsed.toISOString().replace('.000Z', 'Z') !== value) {
    throw new Error(`${label} must be a real UTC timestamp`);
  }
  return parsed.valueOf();
};

const readSnapshot = (evidenceRoot, phase) => {
  const filename = snapshotFilenames[phase];
  const pathname = path.join(evidenceRoot, filename);
  let stats;
  try {
    stats = statSync(pathname);
  } catch (error) {
    throw new Error(`missing runner resource snapshot ${filename}: ${error.message}`);
  }
  if (!stats.isFile()) throw new Error(`${filename} must be a regular file`);
  if (stats.size === 0) throw new Error(`${filename} must not be empty`);
  if (stats.size > snapshotLimitBytes) {
    throw new Error(`${filename} exceeds ${snapshotLimitBytes} bytes`);
  }

  const bytes = readFileSync(pathname);
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch (error) {
    throw new Error(`${filename} must contain valid UTF-8: ${error.message}`);
  }
  if (text.includes('\0')) throw new Error(`${filename} must not contain NUL bytes`);
  const lines = text.replace(/\r\n/gu, '\n').split('\n');
  while (lines.at(-1) === '') lines.pop();

  const capturedAt = requireSingletonValue(lines, 'captured_at', filename);
  const actualPhase = requireSingletonValue(lines, 'phase', filename);
  const runnerLabel = requireSingletonValue(lines, 'runner_label', filename);
  const nprocValue = requireSingletonValue(lines, 'nproc', filename);
  if (actualPhase !== phase) throw new Error(`${filename} phase must be ${phase}`);
  if (!/^\d+$/u.test(nprocValue) || Number.parseInt(nprocValue, 10) <= 0) {
    throw new Error(`${filename} nproc must be a positive integer`);
  }

  const kernelLines = lines.filter((line) => line.startsWith('Linux '));
  if (kernelLines.length !== 1) throw new Error(`${filename} must contain one Linux uname line`);
  if (!lines.some((line) => /^\s*Mem:\s+\d+/u.test(line))) {
    throw new Error(`${filename} must contain a free -b Mem row`);
  }
  if (!lines.some((line) => /^Filesystem\s+/u.test(line))) {
    throw new Error(`${filename} must contain a df -B1 header`);
  }
  if (!lines.some((line) => /^\S+\s+\d+\s+\d+\s+\d+\s+\d+%\s+\/$/u.test(line.trim()))) {
    throw new Error(`${filename} must contain the root filesystem df row`);
  }

  return {
    capturedAt,
    capturedAtMs: requireUtcTimestamp(capturedAt, `${filename} captured_at`),
    runnerLabel,
    nproc: Number.parseInt(nprocValue, 10),
    kernel: kernelLines[0],
  };
};

export const validateRunnerResourceSnapshots = (
  evidenceRoot,
  environment = process.env,
) => {
  if (environment.INDEX_BENCH_REQUIRE_RUNNER_RESOURCES !== '1') return;

  const before = readSnapshot(evidenceRoot, 'before');
  const after = readSnapshot(evidenceRoot, 'after');
  if (before.runnerLabel !== after.runnerLabel) {
    throw new Error('runner resource snapshots must use the same runner_label');
  }
  if (before.nproc !== after.nproc) {
    throw new Error('runner resource snapshots must use the same nproc value');
  }
  if (before.kernel !== after.kernel) {
    throw new Error('runner resource snapshots must use the same Linux uname identity');
  }
  if (before.capturedAtMs > after.capturedAtMs) {
    throw new Error('runner resource snapshots must be ordered before then after');
  }
};
