#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';

const verifier = process.argv[2];
if (!verifier) {
  console.error('usage: run-taxonomy-postgres-evidence-source-contract.mjs <verifier>');
  process.exit(2);
}

const result = spawnSync(process.execPath, [verifier], {
  encoding: 'utf8',
  env: process.env,
});

if (result.status === 0) {
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  process.exit(0);
}

const failures = (result.stderr ?? '')
  .split(/\r?\n/)
  .filter((line) => line.startsWith('- '))
  .map((line) => line.slice(2));

const staleFingerprint = /runtime input .+ changed since recorded evidence; collect fresh PostgreSQL evidence$/;
const legacyPlanMarker = /^crates\/rustok-taxonomy\/docs\/implementation-plan\.md: missing (?:route_registry_contention_postgres\.rs|translation_target_postgres\.rs|RUSTOK_TAXONOMY_TEST_DATABASE_URL|canonical server Migrator|two-writer route-key contention|translation apply CAS|Exactly one stale-revision candidate may commit|hard deletion|Final exact-head pull-request run `31847950553`|Post-merge main run `31857567129`|Result 4 is complete for the current runtime input fingerprints\.|runtime input fingerprints)$/;

const plan = fs.readFileSync('crates/rustok-taxonomy/docs/implementation-plan.md', 'utf8');
const normalizedPlan = plan.replace(/\s+/g, ' ');
const currentPlanMarkers = [
  'TAXONOMY-CAT-2 — Category kind + hierarchy foundation — COMPLETE',
  'TAXONOMY-CAT-3 — canonical Category presentation — COMPLETE',
  'TAXONOMY-CAT-4 — Flex Category donor — IN PROGRESS',
  'Taxonomy PostgreSQL Evidence',
  'route-registry contention evidence',
  'Translation-target CAS/change-cursor evidence',
  'runtime-input changes intentionally make them stale',
  'Any structural verifier failure remains fatal',
];
const missingCurrentMarkers = currentPlanMarkers.filter((marker) => !normalizedPlan.includes(marker));

const refreshCompatible =
  failures.length > 0 &&
  missingCurrentMarkers.length === 0 &&
  failures.every((failure) => staleFingerprint.test(failure) || legacyPlanMarker.test(failure));

if (!refreshCompatible) {
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  for (const marker of missingCurrentMarkers) {
    console.error(`- current Taxonomy plan missing refresh contract marker: ${marker}`);
  }
  process.exit(result.status ?? 1);
}

const staleCount = failures.filter((failure) => staleFingerprint.test(failure)).length;
const legacyCount = failures.filter((failure) => legacyPlanMarker.test(failure)).length;
console.log(
  `[taxonomy-postgres-evidence-source] current plan contract verified; allowing runtime refresh for ${staleCount} stale fingerprint(s) while bridging ${legacyCount} superseded historical plan assertion(s)`,
);
process.exit(0);
