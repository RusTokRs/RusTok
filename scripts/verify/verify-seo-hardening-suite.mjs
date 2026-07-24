#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim()
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, '../..');

const checks = [
  ['verify-seo-fba.mjs', 'SEO FBA contract'],
  ['verify-seo-admin-boundary.mjs', 'SEO admin boundary'],
  ['verify-seo-bulk-batch-reads.mjs', 'SEO bulk batching and bounded execution'],
  ['verify-seo-diagnostics-batch-reads.mjs', 'SEO diagnostics batching'],
  ['verify-seo-sitemap-background-worker.mjs', 'SEO sitemap background worker'],
  ['verify-seo-index-repair-background-worker.mjs', 'SEO index repair background worker'],
  ['verify-seo-entrypoint-authorization.mjs', 'SEO worker and operator authorization'],
  ['verify-seo-failure-classification.mjs', 'SEO failure classification'],
];

const failures = [];
for (const [fileName, label] of checks) {
  const scriptPath = path.join(scriptDir, fileName);
  console.log(`\n▶ ${label}`);
  const result = spawnSync(process.execPath, [scriptPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      RUSTOK_VERIFY_REPO_ROOT: repoRoot,
    },
    encoding: 'utf8',
  });

  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);

  if (result.error) {
    failures.push(`${label}: ${result.error.message}`);
    continue;
  }
  if (result.status !== 0) {
    const outcome = result.signal ? `signal ${result.signal}` : `exit ${result.status}`;
    failures.push(`${label}: ${outcome}`);
  }
}

if (failures.length > 0) {
  console.error('\nSEO hardening suite failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(`\n✔ SEO hardening suite passed (${checks.length} checks)`);
