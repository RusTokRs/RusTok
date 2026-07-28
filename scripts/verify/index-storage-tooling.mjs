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
  node scripts/verify/index-storage-tooling.mjs partition-prepare --input <config.json> --manifest <manifest.json> --bootstrap <bootstrap.sql>
  node scripts/verify/index-storage-tooling.mjs partition-capture [--plan] --manifest <manifest.json> --query-audit <query-audit.json> --root <bundle-directory> [--packet <packet.json>] [--admission <admission.json>]
  node scripts/verify/index-storage-tooling.mjs partition-assemble --manifest <manifest.json> --capture <capture.json> --output <packet.json>
  node scripts/verify/index-storage-tooling.mjs partition-validate --input <packet.json> --output <admission.json>
  node scripts/verify/index-storage-tooling.mjs partition-report --root <bundle-directory> [--packet <packet.json>] [--admission <admission.json>]
  node scripts/verify/index-storage-tooling.mjs hash <comparison.json>
  node scripts/verify/index-storage-tooling.mjs prepare --comparison <comparison.json> --selected <prototype> --owner <owner> --date <YYYY-MM-DD> --output <decision.json> [--force]
  node scripts/verify/index-storage-tooling.mjs render --comparison <comparison.json> --decision <decision.json> --output <adr.md>
  node scripts/verify/index-storage-tooling.mjs verify-adr --comparison <comparison.json> --decision <decision.json> --adr <adr.md>

Commands:
  contract            Run static Index boundary, evidence, partition, standalone-tool, and ADR guards.
  fixtures            Run standalone, evidence, comparator, decision, partition, and ADR fixture suites.
  packet              Validate one smoke, 100k, or 1m storage evidence packet.
  compare             Generate a cross-scale storage comparison.
  partition-prepare   Bind an immutable partition evidence manifest and shadow-only bootstrap SQL.
  partition-capture   Print a no-write preflight plan or run every owner-operated capture, assembly, and validation stage.
  partition-assemble  Build one packet from six retained raw JSON artifacts and exact-byte hashes.
  partition-validate  Validate a measured partition packet and publish calculated admission output.
  partition-report    Recalculate and render a read-only review of all nine retained bundle files.
  hash                Print the SHA-256 digest of the exact comparison.json bytes.
  prepare             Create a non-overwriting manual decision draft bound to exact comparison bytes.
  render              Finalize the manual storage ADR with comparison and decision SHA-256 bindings.
  verify-adr          Verify a saved ADR against exact comparison and decision bytes.`);
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
    'verify-index-storage-migrations.mjs',
    'verify-index-mutation-storage.mjs',
    'verify-index-schema-leases.mjs',
    'verify-index-secondary-index-lifecycle.mjs',
    'verify-index-partition-admission.mjs',
    'verify-index-partition-evidence.mjs',
    'verify-index-partition-snapshot-capture.mjs',
    'verify-index-partition-query-evidence.mjs',
    'verify-index-partition-mutation-evidence.mjs',
    'verify-index-partition-maintenance-evidence.mjs',
    'verify-index-partition-cutover-evidence.mjs',
    'verify-index-partition-full-capture.mjs',
    'verify-index-storage-source-oracle.mjs',
    'verify-index-storage-read-ordering-contract.mjs',
    'verify-index-storage-standalone-tools.mjs',
    'verify-index-storage-adr-tooling.mjs',
    'verify-index-storage-adr-integrity.mjs',
    'verify-index-storage-renderer-lifecycle.mjs',
    'verify-index-storage-adr-verifier-cli.mjs',
    'verify-index-storage-decision-preparation-lifecycle.mjs',
    'verify-index-storage-decision-text-schema.mjs',
    'verify-index-storage-router-arguments.mjs',
    'verify-index-storage-comparator-lifecycle.mjs',
    'verify-index-storage-methodology-envelope.mjs',
    'verify-index-storage-finalizer-lifecycle.mjs',
    'verify-index-storage-placeholder-contract.mjs',
    'verify-index-storage-hash-cli-contract.mjs',
  ]) {
    runScript(script);
  }
};

const runFixtures = (args) => {
  if (args.length !== 0) fail('fixtures does not accept arguments');
  runNode([
    '--test',
    scriptPath('index-storage-tooling-arguments.test.mjs'),
    scriptPath('check-index-storage-read-ordering.test.mjs'),
    scriptPath('index-storage-standalone-tools.test.mjs'),
    scriptPath('compare-index-storage-evidence.test.mjs'),
    scriptPath('compare-index-storage-evidence-lifecycle.test.mjs'),
    scriptPath('comparison-methodology-envelope.test.mjs'),
    scriptPath('hash-index-storage-comparison-cli.test.mjs'),
    scriptPath('render-index-storage-adr.test.mjs'),
    scriptPath('finalize-index-storage-adr-decision-contract.test.mjs'),
    scriptPath('finalize-index-storage-adr-placeholder.test.mjs'),
    scriptPath('index-storage-decision-tooling.test.mjs'),
    scriptPath('prepare-index-storage-decision-lifecycle.test.mjs'),
    scriptPath('storage-decision-schema-text.test.mjs'),
    scriptPath('index-partition-evidence.test.mjs'),
    scriptPath('index-partition-evidence-assembly.test.mjs'),
    scriptPath('index-partition-full-capture-plan.test.mjs'),
    scriptPath('index-partition-review.test.mjs'),
  ], 'Index storage fixture suites');
};

const parsePacketArgs = (args) => {
  let scale = null;
  let root = null;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--scale' && args[index + 1] && !args[index + 1].startsWith('--')) {
      if (scale !== null) fail('--scale was provided more than once');
      scale = args[++index];
    } else if (argument === '--root' && args[index + 1] && !args[index + 1].startsWith('--')) {
      if (root !== null) fail('--root was provided more than once');
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
  const packetRoot = root ?? path.join('evidence/index-storage', scale);
  runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot]);
  const environment = {
    ...process.env,
    INDEX_BENCH_SCALE: scale,
  };
  if (root !== null) environment.INDEX_BENCH_EVIDENCE_ROOT = root;
  runScript('validate-index-storage-evidence.mjs', [], environment);
};

const parseCompareInputs = (args) => {
  const inputs = [];
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') return null;
    if ((argument === '--input' || argument === '--output')
        && args[index + 1] && !args[index + 1].startsWith('--')) {
      if (argument === '--input') inputs.push(args[index + 1]);
      index += 1;
    } else {
      fail(`unknown or incomplete compare argument: ${argument}`);
    }
  }
  if (inputs.length === 0) fail('compare requires at least one --input directory');
  return inputs;
};

const runCompare = (args) => {
  const inputs = parseCompareInputs(args);
  if (inputs !== null) {
    const orderingArgs = inputs.flatMap((input) => ['--input', input]);
    runScript('check-index-storage-read-ordering.mjs', orderingArgs);
  }
  runScript('compare-index-storage-evidence.mjs', args);
};

const [command, ...args] = process.argv.slice(2);
if (!command) {
  usage();
  process.exit(0);
}
if (command === '--help' || command === '-h') {
  if (args.length !== 0) fail('help must be the only argument');
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
    runCompare(args);
    break;
  case 'partition-prepare':
    runScript('prepare-index-partition-evidence.mjs', args);
    break;
  case 'partition-capture':
    runScript('run-index-partition-evidence.mjs', args);
    break;
  case 'partition-assemble':
    runScript('assemble-index-partition-evidence.mjs', args);
    break;
  case 'partition-validate':
    runScript('validate-index-partition-evidence.mjs', args);
    break;
  case 'partition-report':
    runScript('render-index-partition-review.mjs', args);
    break;
  case 'hash':
    runScript('hash-index-storage-comparison.mjs', args);
    break;
  case 'prepare':
    runScript('prepare-index-storage-decision.mjs', args);
    break;
  case 'render':
    runScript('finalize-index-storage-adr.mjs', args);
    break;
  case 'verify-adr':
    runScript('verify-index-storage-adr.mjs', args);
    break;
  default:
    fail(`unknown command: ${command}`);
}
