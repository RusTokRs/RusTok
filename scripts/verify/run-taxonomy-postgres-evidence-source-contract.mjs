#!/usr/bin/env node

import { spawnSync } from 'node:child_process';

const verifier = process.argv[2];
if (!verifier) {
  console.error('usage: run-taxonomy-postgres-evidence-source-contract.mjs <verifier>');
  process.exit(2);
}

const result = spawnSync(process.execPath, [verifier], {
  encoding: 'utf8',
  env: process.env,
});

if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);

if (result.status === 0) {
  process.exit(0);
}

const failures = (result.stderr ?? '')
  .split(/\r?\n/)
  .filter((line) => line.startsWith('- '));

const staleFingerprint = /runtime input .+ changed since recorded evidence; collect fresh PostgreSQL evidence$/;
const refreshOnly = failures.length > 0 && failures.every((line) => staleFingerprint.test(line.slice(2)));

if (!refreshOnly) {
  process.exit(result.status ?? 1);
}

console.log(
  `[taxonomy-postgres-evidence-source] recorded evidence is stale for ${failures.length} runtime input(s); allowing the current-head PostgreSQL runtime job to refresh evidence`,
);
process.exit(0);
