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

const fail = (message) => {
  throw new Error(message);
};

const usage = () => {
  console.log(`Usage:
  node scripts/verify/run-index-partition-evidence.mjs \\
    --manifest <manifest.json> \\
    --query-audit <query-audit.json> \\
    --root <bundle-directory> \\
    [--packet <partition-packet.json>] \\
    [--admission <admission.json>]

The command requires DATABASE_URL and ${fullCaptureOptIn}=1.`);
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
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--manifest', '--query-audit', '--root', '--packet', '--admission'].includes(flag)
        || !value || value.startsWith('--')) {
      fail('expected --manifest, --query-audit, --root, optional --packet, and optional --admission pairs');
    }
    if (values.has(flag)) fail(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  for (const flag of ['--manifest', '--query-audit', '--root']) {
    if (!values.has(flag)) fail(`${flag} is required`);
  }
  const root = path.resolve(values.get('--root'));
  const options = {
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
};

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
    ...rawArtifactNames.map((filename) => path.join(options.root, filename)),
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

const cargoRun = (binary, label, environment) => {
  runCommand(
    process.env.CARGO ?? 'cargo',
    ['run', '-p', 'rustok-benchmarks', '--bin', binary, '--release'],
    label,
    environment,
  );
};

const scriptPath = (filename) => path.join(scriptDirectory, filename);

try {
  const options = parseArgs(process.argv.slice(2));
  if (options === null) {
    usage();
    process.exit(0);
  }
  if (process.env[fullCaptureOptIn] !== '1') {
    fail(`${fullCaptureOptIn}=1 is required because this command runs every owner-operated PostgreSQL evidence stage`);
  }
  if (!process.env.DATABASE_URL) fail('DATABASE_URL is required');
  preflight(options);

  const baseEnvironment = {
    ...process.env,
    INDEX_PARTITION_MANIFEST: options.manifest,
    INDEX_PARTITION_EVIDENCE_ROOT: options.root,
    INDEX_PARTITION_QUERY_OUTPUT: path.join(options.root, 'query.json'),
    INDEX_PARTITION_MUTATION_OUTPUT: path.join(options.root, 'mutation.json'),
    INDEX_PARTITION_MAINTENANCE_OUTPUT: path.join(options.root, 'maintenance.json'),
    INDEX_PARTITION_CUTOVER_OUTPUT: path.join(options.root, 'cutover.json'),
    INDEX_PARTITION_CAPTURE_OUTPUT: options.capture,
  };

  cargoRun(
    'index-partition-snapshot-capture',
    'capture baseline and retained shadow snapshot',
    {
      ...baseEnvironment,
      INDEX_PARTITION_QUERY_AUDIT: options.queryAudit,
      INDEX_PARTITION_ALLOW_SHADOW_COPY: '1',
    },
  );
  cargoRun(
    'index-partition-query-evidence',
    'capture baseline/shadow query evidence',
    { ...baseEnvironment, INDEX_PARTITION_ALLOW_QUERY_EVIDENCE: '1' },
  );
  cargoRun(
    'index-partition-mutation-evidence',
    'capture rollback-only mutation and WAL evidence',
    { ...baseEnvironment, INDEX_PARTITION_ALLOW_MUTATION_EVIDENCE: '1' },
  );
  cargoRun(
    'index-partition-maintenance-evidence',
    'capture ordinary-VACUUM maintenance evidence',
    { ...baseEnvironment, INDEX_PARTITION_ALLOW_MAINTENANCE_EVIDENCE: '1' },
  );
  cargoRun(
    'index-partition-cutover-evidence',
    'capture cutover lock and rollback rehearsal evidence',
    { ...baseEnvironment, INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE: '1' },
  );
  cargoRun(
    'index-partition-capture-finalize',
    'bind raw artifacts to runner and PostgreSQL identity',
    { ...baseEnvironment, INDEX_PARTITION_ALLOW_CAPTURE_FINALIZE: '1' },
  );

  runCommand(
    process.execPath,
    [
      scriptPath('assemble-index-partition-evidence.mjs'),
      '--manifest', options.manifest,
      '--capture', options.capture,
      '--output', options.packet,
    ],
    'assemble exact-byte retained partition packet',
  );
  runCommand(
    process.execPath,
    [
      scriptPath('validate-index-partition-evidence.mjs'),
      '--input', options.packet,
      '--output', options.admission,
    ],
    'validate retained partition packet admission',
  );
  console.log(`${prefix} complete: bundle=${options.root}`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
