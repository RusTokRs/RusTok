#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
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
const runtimeInputPaths = [
  'Cargo.toml',
  'Cargo.lock',
  'crates/rustok-core/Cargo.toml',
  'crates/rustok-core/src',
  'crates/rustok-migrations/Cargo.toml',
  'crates/rustok-migrations/src',
  'crates/rustok-outbox/Cargo.toml',
  'crates/rustok-outbox/src',
  'crates/rustok-taxonomy/Cargo.toml',
  'crates/rustok-taxonomy/src',
  'crates/rustok-taxonomy/tests/route_registry_contention_postgres.rs',
  'crates/rustok-taxonomy/tests/translation_target_postgres.rs',
  'crates/rustok-translation-targets/Cargo.toml',
  'crates/rustok-translation-targets/src',
  '.github/workflows/taxonomy-postgres-evidence.yml',
];
const exactHead = {
  runId: 33407161450,
  headSha: '3c42235867036511eabeb87ed421d9d74b7ff997',
  sourceJobId: 99537462097,
  runtimeJobId: 99537547759,
  gateJobId: 99539081717,
  artifactId: 9763777630,
  artifactName:
    'taxonomy-postgres-evidence-33407161450-3c42235867036511eabeb87ed421d9d74b7ff997',
  artifactDigest: 'sha256:70d124fba72cf14c00f88330639ff8e70200a6f3919963ae0af5cf9760874854',
  artifactExpiresAt: '2026-09-07T15:13:04Z',
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

function workflowCommandValue(value) {
  return String(value).replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
}

function gitObjectId(relativePath) {
  try {
    return execFileSync('git', ['rev-parse', `HEAD:${relativePath}`], {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch (error) {
    failures.push(`${relativePath}: unable to resolve current Git object: ${error.message}`);
    return '';
  }
}

function verifyRuntimeInputSnapshot(evidence, label) {
  const snapshot = evidence.runtime_input_snapshot ?? {};
  const fingerprints = snapshot.git_objects ?? {};

  if (
    snapshot.runtime_commit !== exactHead.headSha ||
    snapshot.validated_through_commit !== exactHead.headSha
  ) {
    failures.push(`${label}: runtime input snapshot provenance drift`);
  }

  const recordedPaths = Object.keys(fingerprints).sort();
  const expectedPaths = [...runtimeInputPaths].sort();
  if (JSON.stringify(recordedPaths) !== JSON.stringify(expectedPaths)) {
    failures.push(`${label}: runtime input snapshot path set drift`);
  }

  for (const relativePath of runtimeInputPaths) {
    const recorded = fingerprints[relativePath];
    if (typeof recorded !== 'string' || !/^[0-9a-f]{40}$/.test(recorded)) {
      failures.push(`${label}: missing Git object fingerprint for ${relativePath}`);
      continue;
    }
    const current = gitObjectId(relativePath);
    if (current && current !== recorded) {
      failures.push(
        `${label}: runtime input ${relativePath} changed since recorded evidence; collect fresh PostgreSQL evidence`,
      );
    }
  }
}

function verifyRun(recorded, expected, label) {
  if (
    recorded.run_id !== expected.runId ||
    recorded.head_sha !== expected.headSha ||
    recorded.source_job_id !== expected.sourceJobId ||
    recorded.runtime_job_id !== expected.runtimeJobId ||
    recorded.gate_job_id !== expected.gateJobId ||
    recorded.conclusion !== 'success' ||
    recorded.artifact_id !== expected.artifactId ||
    recorded.artifact_name !== expected.artifactName ||
    recorded.artifact_digest !== expected.artifactDigest ||
    recorded.artifact_expires_at !== expected.artifactExpiresAt
  ) {
    failures.push(`${label}: runtime provenance drift`);
  }
}

function verifyRefreshRuntimeEvidence(evidence, label) {
  const runtime = evidence.runtime_evidence ?? {};
  const postMerge = runtime.post_merge_main ?? {};

  if (
    runtime.workflow !== 'Taxonomy PostgreSQL Evidence' ||
    runtime.required_backend !== 'PostgreSQL 16'
  ) {
    failures.push(`${label}: runtime evidence environment drift`);
  }

  verifyRun(runtime.exact_head_pull_request ?? {}, exactHead, `${label}: exact-head pull request`);

  if (postMerge.status !== 'pending' || Object.keys(postMerge).length !== 1) {
    failures.push(`${label}: post-merge evidence must remain explicitly pending`);
  }

  verifyRuntimeInputSnapshot(evidence, label);
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
    "wait_event_type = 'Lock'",
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
    evidence.status !== 'runtime_refresh_pending_post_merge' ||
    evidence.compile_policy !== 'ci_runtime_workflow' ||
    evidence.runtime_status !== 'exact_head_passed_post_merge_pending'
  ) {
    failures.push(`${files.evidence}: identity/status drift`);
  }
  if (
    evidence.environment?.primary !== 'RUSTOK_TAXONOMY_TEST_DATABASE_URL' ||
    evidence.environment?.required_backend !== 'postgresql' ||
    evidence.environment?.rust_toolchain !== '1.96.0' ||
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
    JSON.stringify(evidence.remaining_open_result_4_evidence) !==
      JSON.stringify(['post_merge_main_postgresql_evidence'])
  ) {
    failures.push(`${files.evidence}: pending Result 4 evidence drift`);
  }
  verifyRefreshRuntimeEvidence(evidence, files.evidence);
}

requireMarkers(
  workflow,
  [
    'name: Taxonomy PostgreSQL Evidence',
    'RUST_TOOLCHAIN: "1.96.0"',
    'DATABASE_URL:',
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL:',
    'image: postgres:16',
    'ref: ${{ github.event.pull_request.head.sha || github.sha }}',
    'Activate exact Rust evidence toolchain',
    'rustup override set',
    'Apply canonical server migrations',
    'cargo run --locked -p rustok-migrations --bin rustok-migrate -- up',
    'cargo test --locked -p rustok-taxonomy --test route_registry_contention_postgres -- --nocapture',
    'cargo test --locked -p rustok-taxonomy --test translation_target_postgres -- --nocapture',
    'Archive Taxonomy PostgreSQL evidence',
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
    'Fresh exact-head refresh run `33407161450`',
    'Result 4 refresh is pending post-merge main evidence.',
    'runtime input fingerprints',
  ],
  files.plan,
);

if (failures.length > 0) {
  console.error('[verify-taxonomy-route-registry-postgres-contention] FAIL');
  for (const failure of failures) {
    console.error(`- ${failure}`);
    if (process.env.GITHUB_ACTIONS === 'true') {
      console.error(`::error title=Taxonomy route evidence::${workflowCommandValue(failure)}`);
    }
  }
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-taxonomy-route-registry-postgres-contention] PASS source=canonical-migrator+harness+evidence+workflow runtime=exact-head-recorded+post-merge-pending+fingerprinted',
);
