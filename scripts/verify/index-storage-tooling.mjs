#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const prefix = '[index-storage-tooling]';
const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));

const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const usage = () => {
  console.log(`Usage:
  node scripts/verify/index-storage-tooling.mjs contract
  node scripts/verify/index-storage-tooling.mjs fixtures
  node scripts/verify/index-storage-tooling.mjs packet --scale <smoke|100k|1m> [--root <directory>]
  node scripts/verify/index-storage-tooling.mjs compare --input <directory> [--input <directory>] [--output <directory>]
  node scripts/verify/index-storage-tooling.mjs hash <comparison.json>
  node scripts/verify/index-storage-tooling.mjs render --comparison <comparison.json> --decision <decision.json> --output <adr.md>

Commands:
  contract  Run static Index boundary, source-oracle/evidence, and ADR tooling guards.
  fixtures Run the comparator and ADR renderer fixture suites.
  packet   Validate one smoke, 100k, or 1m evidence packet through the canonical validator.
  compare  Generate a cross-scale comparison from validated packet directories.
  hash     Print the SHA-256 digest of the exact comparison.json bytes.
  render   Render the manual storage ADR after commit and SHA-256 binding checks.`);
};

const runNode = (args, label, environment = process.env) => {
  const result = spawnSync(process.execPath, args, {
    stdio: 'inherit',
    env: environment,
  });
  if (result.error) fail(`failed to start ${label}: ${result.error.message}`);
  if (result.signal) fail(`${label} terminated by signal ${result.signal}`);
  if (result.status !== 0) process.exit(result.status ?? 1);
};

const scriptPath = (filename) => path.join(scriptDirectory, filename);
const runScript = (filename, args = [], environment = process.env) => {
  runNode([scriptPath(filename), ...args], filename, environment);
};

const runContract = (args) => {
  if (args.length !== 0) fail('contract does not accept arguments');
  for (const script of [
    'verify-index-fba.mjs',
    'verify-index-storage-source-oracle.mjs',
    'verify-index-storage-adr-tooling.mjs',
  ]) {
    runScript(script);
  }
};

const runFixtures = (args) => {
  if (args.length !== 0) fail('fixtures does not accept arguments');
  runNode([
    '--test',
    scriptPath('compare-index-storage-evidence.test.mjs'),
    scriptPath('render-index-storage-adr.test.mjs'),
  ], 'Index storage fixture suites');
};

const parsePacketArgs = (args) => {
  let scale = null;
  let root = null;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--scale' && args[index + 1]) {
      scale = args[++index];
    } else if (argument === '--root' && args[index + 1]) {
      root = args[++index];
    } else {
      fail(`unknown or incomplete packet argument: ${argument}`);
    }
  }
  if (!['smoke', '100k', '1m'].includes(scale)) {
    fail('packet --scale must be smoke, 100k, or 1m');
  }
  return { scale, root };
};

const runPacket = (args) => {
  const { scale, root } = parsePacketArgs(args);
  const environment = {
    ...process.env,
    INDEX_BENCH_SCALE: scale,
  };
  if (root !== null) environment.INDEX_BENCH_EVIDENCE_ROOT = root;
  runScript('validate-index-storage-evidence.mjs', [], environment);
};

const [command, ...args] = process.argv.slice(2);
if (!command || command === '--help' || command === '-h') {
  usage();
  process.exit(0);
}

switch (command) {
  case 'contract':
    runContract(args);
    break;
  case 'fixtures':
    runFixtures(args);
    break;
  case 'packet':
    runPacket(args);
    break;
  case 'compare':
    runScript('compare-index-storage-evidence.mjs', args);
    break;
  case 'hash':
    runScript('hash-index-storage-comparison.mjs', args);
    break;
  case 'render':
    runScript('render-index-storage-adr.mjs', args);
    break;
  default:
    fail(`unknown command: ${command}`);
}
