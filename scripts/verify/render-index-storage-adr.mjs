#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const prefix = '[render-index-storage-adr]';
const allowedArguments = new Set(['--comparison', '--decision', '--output']);
const corePath = fileURLToPath(new URL('./render-index-storage-adr-core.mjs', import.meta.url));
const delegatedCoreContractMarkers = Object.freeze([
  "import { createHash } from 'node:crypto'",
  "const prototypes = ['jsonb', 'typed_eav', 'hot_projection']",
  "'same_result_digest_contract'",
  "from './index-storage-database-settings-contract.mjs'",
  'const readComparison = (filename) =>',
  "createHash('sha256').update(bytes).digest('hex')",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
  'if (comparison.decision_ready !== true)',
  'comparison.methodology?.automatic_winner_selection !== false',
  'comparison must contain exactly one ${scale} evidence entry',
  'comparison decision contract ${field} is not satisfied',
  'decision.comparison_commit must match the evidence comparison commit',
  'decision.comparison_sha256 must be a SHA-256 digest',
  'decision.comparison_sha256 must match the exact comparison.json bytes',
  'decision.rejection_rationales must contain exactly',
  'comparison.cross_scale_ratios',
  'cross-scale prototype order',
  'read workload order differs across scales',
  'mutation workload order differs across scales',
  'const render = (comparison, decision, comparisonSha256) =>',
  'Comparison SHA-256:',
  'renderer does not infer or rank a winning prototype',
]);

const fail = (message) => {
  throw new Error(message);
};

const usage = () => {
  console.log(
    'Usage: node scripts/verify/render-index-storage-adr.mjs '
    + '--comparison <comparison.json> --decision <decision.json> --output <adr.md>',
  );
};

const parseArgs = () => {
  const values = new Map();
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      if (args.length !== 1) fail('help must be the only argument');
      usage();
      return null;
    }
    if (!allowedArguments.has(argument)
        || !args[index + 1]
        || args[index + 1].startsWith('--')) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    if (values.has(argument)) fail(`${argument} was provided more than once`);
    values.set(argument, args[++index]);
  }
  for (const argument of allowedArguments) {
    if (!values.has(argument)) fail(`${argument} is required`);
  }
  return {
    comparison: values.get('--comparison'),
    decision: values.get('--decision'),
    output: values.get('--output'),
  };
};

const requireDelegatedCoreContract = () => {
  const core = readFileSync(corePath, 'utf8');
  for (const marker of delegatedCoreContractMarkers) {
    if (!core.includes(marker)) fail(`renderer core is missing contract marker: ${marker}`);
  }
};

const runCore = (args, stagedOutput) => {
  const result = spawnSync(process.execPath, [
    corePath,
    '--comparison', args.comparison,
    '--decision', args.decision,
    '--output', stagedOutput,
  ], { encoding: 'utf8' });
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error) fail(`renderer core failed to start: ${result.error.message}`);
  if (result.signal) fail(`renderer core terminated by signal ${result.signal}`);
  if (result.status === null) fail('renderer core did not return an exit status');
  return result.status;
};

const main = () => {
  const args = parseArgs();
  if (args === null) return 0;

  const resolvedOutput = path.resolve(args.output);
  for (const [label, filename] of [['comparison', args.comparison], ['decision', args.decision]]) {
    if (resolvedOutput === path.resolve(filename)) {
      fail(`--output must not overwrite the ${label} input`);
    }
  }

  requireDelegatedCoreContract();
  const parent = path.dirname(args.output);
  if (parent && parent !== '.') mkdirSync(parent, { recursive: true });
  const stagingRoot = mkdtempSync(path.join(parent || '.', `.${path.basename(args.output)}.tmp-`));
  const stagedOutput = path.join(stagingRoot, path.basename(args.output));
  try {
    rmSync(args.output, { force: true });
    const status = runCore(args, stagedOutput);
    if (status !== 0) return status;
    if (!existsSync(stagedOutput)) fail('renderer core succeeded without producing staged ADR output');
    renameSync(stagedOutput, args.output);
    console.log(`${prefix} wrote ${args.output}`);
    return 0;
  } finally {
    rmSync(stagingRoot, { recursive: true, force: true });
  }
};

try {
  process.exitCode = main();
} catch (error) {
  console.error(`${prefix} ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
