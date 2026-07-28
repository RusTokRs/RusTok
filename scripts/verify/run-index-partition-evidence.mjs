#!/usr/bin/env node

import { existsSync, lstatSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const prefix = '[run-index-partition-evidence]';
const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const fullCaptureOptIn = 'INDEX_PARTITION_ALLOW_FULL_CAPTURE';
const rawArtifactNames = [
  'baseline.json',
  'shadow.json',
  'query.json',
  'mutation.json',
  'maintenance.json',
  'cutover.json',
];
const pairFlags = new Set([
  '--manifest',
  '--query-audit',
  '--root',
  '--packet',
  '--admission',
]);

const fail = (message) => {
  throw new Error(message);
};

const usage = () => {
  console.log(`Usage:
  node scripts/verify/run-index-partition-evidence.mjs \\
    [--plan] \\
    --manifest <manifest.json> \\
    --query-audit <query-audit.json> \\
    --root <bundle-directory> \\
    [--packet <partition-packet.json>] \\
    [--admission <admission.json>]

The command requires DATABASE_URL and ${fullCaptureOptIn}=1.
--plan performs the same filesystem and environment preflight, prints a
machine-readable execution plan, and starts no PostgreSQL, Cargo, or Node stage.`);
};

const ensureInsideRoot = (root, filename, label) => {
  const relative = path.relative(root, filename);
  if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    fail(`${label} must stay inside the evidence root`);
  }
};

const parseArgs = (args) => {
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) return null;

  const values = new Map();
  let plan = false;
  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    if (['--help', '-h'].includes(flag)) {
      fail('help must be the only argument');
    }
    if (flag === '--plan') {
      if (plan) fail('--plan was provided more than once');
      plan = true;
      continue;
    }
    if (!pairFlags.has(flag)) {
      fail(`unknown partition capture argument: ${flag}`);
    }
    const value = args[index + 1];
    if (!value || value.startsWith('--')) {
      fail(`${flag} requires a value`);
    }
    if (values.has(flag)) fail(`${flag} was provided more than once`);
    values.set(flag, value);
    index += 1;
  }

  for (const flag of ['--manifest', '--query-audit', '--root']) {
    if (!values.has(flag)) fail(`${flag} is required`);
  }
  const root = path.resolve(values.get('--root'));
  const options = {
    plan,
    manifest: path.resolve(values.get('--manifest')),
    queryAudit: path.resolve(values.get('--query-audit')),
    root,
    capture: path.join(root, 'capture.json'),
    packet: path.resolve(values.get('--packet') ?? path.join(root, 'partition-packet.json')),
    admission: path.resolve(values.get('--admission') ?? path.join(root, 'admission.json')),
  };
  if (new Set([
    options.manifest,
    options.queryAudit,
    options.capture,
    options.packet,
    options.admission,
  ]).size !== 5) {
    fail('manifest, query audit, capture, packet, and admission paths must be distinct');
  }
  ensureInsideRoot(root, options.capture, 'capture output');
  ensureInsideRoot(root, options.packet, 'packet output');
  ensureInsideRoot(root, options.admission, 'admission output');
  return options;
};

const ensureRegularFile = (filename, label) => {
  let stat;
  try {
    stat = lstatSync(filename);
  } catch (error) {
    fail(`${label} is unavailable: ${error.message}`);
  }
  if (stat.isSymbolicLink() || !stat.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  if (stat.size === 0) fail(`${label} must not be empty`);
};

const outputContract = (options) => ({
  raw: Object.fromEntries(
    rawArtifactNames.map((filename) => [filename, path.join(options.root, filename)]),
  ),
  capture: options.capture,
  packet: options.packet,
  admission: options.admission,
});

const preflight = (options) => {
  ensureRegularFile(options.manifest, 'manifest');
  ensureRegularFile(options.queryAudit, 'query audit');
  if (existsSync(options.root)) {
    const rootStat = lstatSync(options.root);
    if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
      fail('evidence root must be a regular non-symlink directory');
    }
  }
  const outputs = [
    ...Object.values(outputContract(options).raw),
    options.capture,
    options.packet,
    options.admission,
  ];
  for (const output of outputs) {
    if (existsSync(output)) {
      fail(`refusing to reuse partial partition evidence output: ${output}`);
    }
  }
};

const runCommand = (command, args, label, environment = process.env) => {
  console.log(`${prefix} ${label}`);
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    env: environment,
  });
  if (result.error) fail(`failed to start ${label}: ${result.error.message}`);
  if (result.signal) fail(`${label} terminated by signal ${result.signal}`);
  if (result.status !== 0) fail(`${label} exited with status ${result.status ?? 'unknown'}`);
};

const scriptPath = (filename) => path.join(scriptDirectory, filename);

const baseEnvironmentOverrides = [
  'INDEX_PARTITION_MANIFEST',
  'INDEX_PARTITION_EVIDENCE_ROOT',
  'INDEX_PARTITION_QUERY_OUTPUT',
  'INDEX_PARTITION_MUTATION_OUTPUT',
  'INDEX_PARTITION_MAINTENANCE_OUTPUT',
  'INDEX_PARTITION_CUTOVER_OUTPUT',
  'INDEX_PARTITION_CAPTURE_OUTPUT',
];

const buildBaseEnvironment = (options) => ({
  ...process.env,
  INDEX_PARTITION_MANIFEST: options.manifest,
  INDEX_PARTITION_EVIDENCE_ROOT: options.root,
  INDEX_PARTITION_QUERY_OUTPUT: path.join(options.root, 'query.json'),
  INDEX_PARTITION_MUTATION_OUTPUT: path.join(options.root, 'mutation.json'),
  INDEX_PARTITION_MAINTENANCE_OUTPUT: path.join(options.root, 'maintenance.json'),
  INDEX_PARTITION_CUTOVER_OUTPUT: path.join(options.root, 'cutover.json'),
  INDEX_PARTITION_CAPTURE_OUTPUT: options.capture,
});

const buildStages = (options) => {
  const baseEnvironment = buildBaseEnvironment(options);
  const cargoCommand = process.env.CARGO ?? 'cargo';
  const cargoStages = [
    {
      identifier: 'index-partition-snapshot-capture',
      label: 'capture baseline and retained shadow snapshot',
      outputs: ['baseline.json', 'shadow.json'],
      environment: {
        INDEX_PARTITION_QUERY_AUDIT: options.queryAudit,
        INDEX_PARTITION_ALLOW_SHADOW_COPY: '1',
      },
    },
    {
      identifier: 'index-partition-query-evidence',
      label: 'capture baseline/shadow query evidence',
      outputs: ['query.json'],
      environment: { INDEX_PARTITION_ALLOW_QUERY_EVIDENCE: '1' },
    },
    {
      identifier: 'index-partition-mutation-evidence',
      label: 'capture rollback-only mutation and WAL evidence',
      outputs: ['mutation.json'],
      environment: { INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE: '1' },
    },
    {
      identifier: 'index-partition-maintenance-evidence',
      label: 'capture ordinary-VACUUM maintenance evidence',
      outputs: ['maintenance.json'],
      environment: { INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE: '1' },
    },
    {
      identifier: 'index-partition-cutover-evidence',
      label: 'capture cutover lock and rollback rehearsal evidence',
      outputs: ['cutover.json'],
      environment: { INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE: '1' },
    },
    {
      identifier: 'index-partition-capture-finalize',
      label: 'bind raw artifacts to runner and PostgreSQL identity',
      outputs: ['capture.json'],
      environment: { INDEX_PARTITION_ALLOW_CAPTURE_FINALIZE: '1' },
    },
  ].map((stage) => ({
    kind: 'cargo',
    identifier: stage.identifier,
    label: stage.label,
    command: cargoCommand,
    args: ['run', '-p', 'rustok-benchmarks', '--bin', stage.identifier, '--release'],
    environment: { ...baseEnvironment, ...stage.environment },
    environmentOverrides: [...baseEnvironmentOverrides, ...Object.keys(stage.environment)],
    outputs: stage.outputs.map((filename) => path.join(options.root, filename)),
  }));

  return [
    ...cargoStages,
    {
      kind: 'node',
      identifier: 'assemble-index-partition-evidence.mjs',
      label: 'assemble exact-byte retained partition packet',
      command: process.execPath,
      args: [
        scriptPath('assemble-index-partition-evidence.mjs'),
        '--manifest', options.manifest,
        '--capture', options.capture,
        '--output', options.packet,
      ],
      environment: process.env,
      environmentOverrides: [],
      outputs: [options.packet],
    },
    {
      kind: 'node',
      identifier: 'validate-index-partition-evidence.mjs',
      label: 'validate retained partition packet admission',
      command: process.execPath,
      args: [
        scriptPath('validate-index-partition-evidence.mjs'),
        '--input', options.packet,
        '--output', options.admission,
      ],
      environment: process.env,
      environmentOverrides: [],
      outputs: [options.admission],
    },
  ];
};

const printPlan = (options, stages) => {
  const plan = {
    contract: 'index_partition_full_capture_plan_v1',
    mode: 'plan',
    preflight_completed: true,
    database_connection_attempted: false,
    writes_performed: false,
    required_environment: [fullCaptureOptIn, 'DATABASE_URL'],
    inputs: {
      manifest: options.manifest,
      query_audit: options.queryAudit,
    },
    root: options.root,
    outputs: outputContract(options),
    stages: stages.map((stage, index) => ({
      order: index + 1,
      kind: stage.kind,
      identifier: stage.identifier,
      command: [stage.command, ...stage.args],
      environment_overrides: stage.environmentOverrides,
      outputs: stage.outputs,
    })),
    limitations: [
      'DATABASE_URL presence is checked, but no database connection is opened.',
      'No Cargo or Node evidence stage is started.',
      'No bundle directory or output file is created.',
    ],
  };
  console.log(JSON.stringify(plan, null, 2));
};

const main = () => {
  const options = parseArgs(process.argv.slice(2));
  if (options === null) {
    usage();
    return;
  }
  if (process.env[fullCaptureOptIn] !== '1') {
    fail(`${fullCaptureOptIn}=1 is required because this command covers every owner-operated PostgreSQL evidence stage`);
  }
  if (!process.env.DATABASE_URL) fail('DATABASE_URL is required');
  preflight(options);

  const stages = buildStages(options);
  if (options.plan) {
    printPlan(options, stages);
    return;
  }

  for (const stage of stages) {
    runCommand(stage.command, stage.args, stage.label, stage.environment);
  }
  console.log(`${prefix} complete: bundle=${options.root}`);
};

try {
  main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
