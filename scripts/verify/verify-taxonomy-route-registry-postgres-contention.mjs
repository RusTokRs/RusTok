#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT || '.');
const failures = [];
const files = {
  test: 'crates/rustok-taxonomy/tests/route_registry_contention_postgres.rs',
  evidence:
    'crates/rustok-taxonomy/contracts/evidence/taxonomy-route-registry-postgres-contention.json',
  workflow: '.github/workflows/taxonomy-postgres-evidence.yml',
  plan: 'crates/rustok-taxonomy/docs/implementation-plan.md',
};

function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!fs.existsSync(target)) {
    failures.push(`${relativePath}: missing file`);
    return '';
  }
  return fs.readFileSync(target, 'utf8');
}

function readJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireMarkers(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function forbidMarkers(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const test = read(files.test);
const evidence = readJson(files.evidence);
const workflow = read(files.workflow);
const plan = read(files.plan);

requireMarkers(
  test,
  [
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL',
    'PostgresTaxonomyRouteContentionDb',
    'TaxonomyModule.migrations()',
    'CREATE SCHEMA',
    'SET search_path TO',
    'isolated_connection',
    'WORKER_A_APPLICATION_NAME',
    'WORKER_B_APPLICATION_NAME',
    'Arc::new(Barrier::new(2))',
    'SELECT id FROM taxonomy_term_translations',
    'FOR UPDATE',
    'wait_event_type = \'Lock\'',
    'update_module_term_in_tx(',
    'CONTESTED_ROUTE_KEY',
    'success_count, 1',
    'claimed concurrently',
    'the contested route key must have one durable owner',
    'winner translation and route reservation must commit together',
    'loser translation update must roll back',
    'one localized route identity must have exactly one owner',
    'DROP SCHEMA IF EXISTS',
  ],
  files.test,
);
forbidMarkers(
  test,
  [
    'INSERT INTO taxonomy_term_route_keys',
    'taxonomy_term_route_key::ActiveModel',
    'reconcile_route_keys_for_locale_in_tx',
  ],
  files.test,
);

if (evidence) {
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== 'taxonomy' ||
    evidence.surface !== 'route_registry_postgres_contention' ||
    evidence.status !== 'executable_no_runtime_record' ||
    evidence.compile_policy !== 'ci_runtime_workflow' ||
    evidence.runtime_status !== 'not_recorded'
  ) {
    failures.push(`${files.evidence}: identity/status drift`);
  }
  if (
    evidence.environment?.primary !== 'RUSTOK_TAXONOMY_TEST_DATABASE_URL' ||
    evidence.environment?.required_backend !== 'postgresql' ||
    evidence.test_target !== files.test ||
    evidence.source_guardrail !==
      'scripts/verify/verify-taxonomy-route-registry-postgres-contention.mjs' ||
    evidence.runtime_workflow !== files.workflow
  ) {
    failures.push(`${files.evidence}: environment/source/runtime contract drift`);
  }
  const contract = evidence.production_contract ?? {};
  for (const key of [
    'two_independent_writer_connections',
    'both_writers_complete_route_preflight_before_release',
    'contention_is_forced_after_translation_row_prelock',
    'route_registry_primary_key_is_storage_authority',
    'exactly_one_writer_commits',
    'losing_writer_reports_concurrent_route_claim',
    'losing_translation_update_rolls_back',
    'winner_translation_and_route_reservation_commit_together',
    'exactly_one_durable_route_owner',
  ]) {
    if (contract[key] !== true) failures.push(`${files.evidence}: ${key} drift`);
  }
  if (contract.mutation_path !== 'update_module_term_in_tx') {
    failures.push(`${files.evidence}: mutation_path drift`);
  }
  if (
    !Array.isArray(evidence.remaining_open_result_4_evidence) ||
    evidence.remaining_open_result_4_evidence.length !== 3
  ) {
    failures.push(`${files.evidence}: remaining Open result 4 evidence drift`);
  }
}

requireMarkers(
  workflow,
  [
    'name: Taxonomy PostgreSQL Evidence',
    'DATABASE_URL:',
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL:',
    'image: postgres:16',
    'Apply canonical server migrations',
    'cargo run --locked -p rustok-migrations --bin rustok-migrate -- up',
    'cargo test --locked -p rustok-taxonomy --test route_registry_contention_postgres -- --nocapture',
    'cargo test --locked -p rustok-taxonomy --test translation_target_postgres -- --nocapture',
    'Taxonomy PostgreSQL Evidence Gate',
  ],
  files.workflow,
);

requireMarkers(
  plan,
  [
    'route_registry_contention_postgres.rs',
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL',
    'canonical server Migrator',
    'two-writer route-key contention',
    'translation apply CAS',
    'change-cursor',
  ],
  files.plan,
);

if (failures.length > 0) {
  console.error('[verify-taxonomy-route-registry-postgres-contention] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-taxonomy-route-registry-postgres-contention] PASS source=canonical-migrator+harness+evidence+workflow runtime=not-recorded',
);
