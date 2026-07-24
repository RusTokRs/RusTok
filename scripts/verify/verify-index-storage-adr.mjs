#!/usr/bin/env node

import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const prefix = '[verify-index-storage-adr]';
const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));

const fail = (message) => {
  throw new Error(message);
};

const usage = () => {
  console.log(
    'Usage: node scripts/verify/verify-index-storage-adr.mjs '
    + '--comparison <comparison.json> --decision <decision.json> --adr <adr.md>',
  );
};

const parseArgs = () => {
  const values = new Map();
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      usage();
      return null;
    }
    if (!argument.startsWith('--') || !args[index + 1] || args[index + 1].startsWith('--')) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    if (values.has(argument)) fail(`${argument} was provided more than once`);
    values.set(argument, args[++index]);
  }
  for (const argument of ['--comparison', '--decision', '--adr']) {
    if (!values.has(argument)) fail(`${argument} is required`);
  }
  return {
    comparison: values.get('--comparison'),
    decision: values.get('--decision'),
    adr: values.get('--adr'),
  };
};

const readBytes = (filename, label) => {
  if (!existsSync(filename)) fail(`missing ${label}: ${filename}`);
  return readFileSync(filename);
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

const requireDigestLine = (markdown, label, expected) => {
  const expression = new RegExp(`^- ${label} SHA-256: \\`([0-9a-f]{64})\\`$`, 'gmu');
  const matches = [...markdown.matchAll(expression)];
  if (matches.length !== 1) fail(`ADR must contain exactly one ${label} SHA-256 line`);
  if (matches[0][1] !== expected) {
    fail(`ADR ${label} SHA-256 does not match the exact input bytes`);
  }
};

const main = () => {
  const args = parseArgs();
  if (args === null) return;
  const comparisonBytes = readBytes(args.comparison, 'comparison');
  const decisionBytes = readBytes(args.decision, 'decision');
  const adrBytes = readBytes(args.adr, 'ADR');
  const markdown = adrBytes.toString('utf8');

  requireDigestLine(markdown, 'Comparison', sha256(comparisonBytes));
  requireDigestLine(markdown, 'Decision', sha256(decisionBytes));

  const temporaryRoot = mkdtempSync(path.join(tmpdir(), 'rustok-index-storage-adr-verify-'));
  try {
    const renderedPath = path.join(temporaryRoot, 'adr.md');
    const result = spawnSync(process.execPath, [
      path.join(scriptDirectory, 'finalize-index-storage-adr.mjs'),
      '--comparison', args.comparison,
      '--decision', args.decision,
      '--output', renderedPath,
    ], { encoding: 'utf8' });
    if (result.error) fail(`failed to start ADR finalizer: ${result.error.message}`);
    if (result.signal) fail(`ADR finalizer terminated by signal ${result.signal}`);
    if (result.status !== 0) {
      const detail = result.stderr?.trim() || result.stdout?.trim() || `exit status ${result.status}`;
      fail(`ADR finalizer failed during verification: ${detail}`);
    }
    const rerendered = readFileSync(renderedPath);
    if (!adrBytes.equals(rerendered)) {
      fail('ADR bytes differ from deterministic finalization of the supplied comparison and decision');
    }
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }

  console.log(`${prefix} verified ${args.adr}`);
};

try {
  main();
} catch (error) {
  console.error(`${prefix} ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
