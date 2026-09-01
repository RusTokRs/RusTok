#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT || '.');
const failures = [];
const files = {
  test: 'crates/rustok-taxonomy/tests/translation_target_postgres.rs',
  evidence:
    'crates/rustok-taxonomy/contracts/evidence/taxonomy-translation-target-postgres.json',
  workflow: '.github/workflows/taxonomy-postgres-evidence.yml',
  serverMigrator: 'crates/rustok-migrations/src/lib.rs',
  provider: 'crates/rustok-taxonomy/src/translation_target.rs',
  owner: 'crates/rustok-taxonomy/src/services.rs',
  changeWriter: 'crates/rustok-taxonomy/src/translation_evidence.rs',
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
const postMergeMain = {
  runId: 33420044854,
  headSha: '0ebb3f53aaa2b9ef62be7fea56cb462dcebbdcf6',
  sourceJobId: 99579915117,
  runtimeJobId: 99580063917,
  gateJobId: 99581484039,
  artifactId: 9768688596,
  artifactName:
    'taxonomy-postgres-evidence-33420044854-0ebb3f53aaa2b9ef62be7fea56cb462dcebbdcf6',
  artifactDigest: 'sha256:bed3651b428a2e779cec8faca0bef804b079655608982ddb3de922549f195303',
  artifactExpiresAt: '2026-09-07T17:31:39Z',
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

function normalizeWhitespace(source) {
  return source.replace(/\s+/g, ' ').trim();
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
    snapshot.runtime_commit !== postMergeMain.headSha ||
    snapshot.validated_through_commit !== postMergeMain.headSha
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

function verifyRecordedRuntimeEvidence(evidence, label) {
  const runtime = evidence.runtime_evidence ?? {};

  if (
    runtime.workflow !== 'Taxonomy PostgreSQL Evidence' ||
    runtime.required_backend !== 'PostgreSQL 16'
  ) {
    failures.push(`${label}: runtime evidence environment drift`);
  }

  verifyRun(runtime.exact_head_pull_request ?? {}, exactHead, `${label}: exact-head pull request`);
  verifyRun(runtime.post_merge_main ?? {}, postMergeMain, `${label}: post-merge main`);
  verifyRuntimeInputSnapshot(evidence, label);
}

const test = read(files.test);
const evidence = readJson(files.evidence);
const workflow = read(files.workflow);
const serverMigrator = read(files.serverMigrator);
const provider = read(files.provider);
const owner = read(files.owner);
const changeWriter = read(files.changeWriter);
const normalizedPlan = normalizeWhitespace(read(files.plan));

requireMarkers(
  test,
  [
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL',
    'REQUIRED_CANONICAL_TABLES',
    'owner_operation_receipts',
    'taxonomy_terms',
    'taxonomy_term_translations',
    'taxonomy_translation_changes',
    'taxonomy_term_route_keys',
    'ensure_canonical_schema',
    'SELECT to_regclass($1) IS NOT NULL AS present',
    '.max_connections(1)',
    'isolated_connection',
    'concurrent_same_revision_translation_applies_commit_once',
    'Arc::new(Barrier::new(candidates.len()))',
    'candidate_provider.apply_patch(context, patch)',
    'exactly one same-revision Taxonomy apply must commit',
    'PortErrorKind::Conflict',
    'final_snapshot.summary.resource_revision.as_str(), "3"',
    'resource_revision.as_str() == "3"',
    'change_cursor_resumes_after_provider_reconstruction_and_delete',
    'after: Some(first_cursor.clone())',
    'after: Some(second_cursor.clone())',
    'delete_term(tenant_id, term_id, admin())',
    'TranslationResourceLifecycle::Deleted',
    'assert!(drained.changes.is_empty())',
    'progress.owner_change_cursor, Some(deleted_cursor)',
    'tokio::time::sleep(Duration::from_millis(2))',
    'does not claim arbitrary concurrent transaction commit ordering',
  ],
  files.test,
);
forbidMarkers(
  test,
  [
    'OutboxModule.migrations()',
    'TaxonomyModule.migrations()',
    'CREATE SCHEMA',
    'SET search_path TO',
    'apply_exact_translation_in_tx(',
    'translation_change::ActiveModel',
  ],
  files.test,
);

const rawStatementCount = (test.match(/Statement::from_sql_and_values\(/g) ?? []).length;
if (rawStatementCount !== 1) {
  failures.push(
    `${files.test}: expected exactly one raw SQL statement for the canonical schema probe, found ${rawStatementCount}`,
  );
}

if (evidence) {
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== 'taxonomy' ||
    evidence.surface !== 'translation_target_postgres' ||
    evidence.status !== 'runtime_recorded' ||
    evidence.runtime_status !== 'passed'
  ) {
    failures.push(`${files.evidence}: identity/status drift`);
  }
  if (
    evidence.environment?.primary !== 'RUSTOK_TAXONOMY_TEST_DATABASE_URL' ||
    evidence.environment?.required_backend !== 'postgresql' ||
    evidence.environment?.rust_toolchain !== '1.96.0' ||
    evidence.test_target !== files.test ||
    evidence.source_guardrail !==
      'scripts/verify/verify-taxonomy-translation-target-postgres.mjs' ||
    evidence.runtime_workflow !== files.workflow
  ) {
    failures.push(`${files.evidence}: environment/source/runtime contract drift`);
  }
  const contract = evidence.production_contract ?? {};
  for (const key of [
    'canonical_server_migrations',
    'owner_operation_receipts_from_server_migrator',
    'retained_taxonomy_migrations',
    'canonical_schema_required_by_test_target',
    'same_exact_snapshot_for_concurrent_candidates',
    'same_resource_source_target_revisions_for_concurrent_candidates',
    'independent_database_connections',
    'distinct_idempotency_keys',
    'exactly_one_same_revision_apply_commits',
    'stale_competitor_closes_as_conflict',
    'resource_revision_advances_once',
    'target_revision_advances_once',
    'exactly_one_winning_change_fact_is_durable',
    'cursor_resumes_after_provider_reconstruction',
    'hard_delete_emits_deleted_lifecycle_change',
    'cursor_drains_after_latest_change',
    'progress_cursor_matches_latest_durable_change',
    'concurrent_commit_order_is_not_claimed',
  ]) {
    if (contract[key] !== true) failures.push(`${files.evidence}: ${key} drift`);
  }
  if (
    !Array.isArray(evidence.remaining_open_result_4_evidence) ||
    evidence.remaining_open_result_4_evidence.length !== 0
  ) {
    failures.push(`${files.evidence}: Result 4 must not retain open evidence items`);
  }
  verifyRecordedRuntimeEvidence(evidence, files.evidence);
}

requireMarkers(
  workflow,
  [
    'name: Taxonomy PostgreSQL Evidence',
    'RUST_TOOLCHAIN: "1.96.0"',
    'image: postgres:16',
    'persist-credentials: false',
    'ref: ${{ github.event.pull_request.head.sha || github.sha }}',
    'Activate exact Rust evidence toolchain',
    'rustup override set',
    'Verify route-registry PostgreSQL contention source',
    'Verify translation-target PostgreSQL source',
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
  serverMigrator,
  [
    'm20260803_000001_create_owner_operation_receipts',
    'Box::new(m20260803_000001_create_owner_operation_receipts::Migration)',
    'all.extend(rustok_taxonomy::migrations::migrations())',
    'sort_migrations_by_dependencies',
    'validate_migration_dependency_order',
  ],
  files.serverMigrator,
);
requireMarkers(
  provider,
  [
    'idempotency::admit(',
    'apply_exact_translation_in_tx(',
    'async fn read_changes(',
    'order_by_asc(ChangeColumn::Id)',
    'filter(ChangeColumn::Id.gt(after))',
    'PROGRESS_STABILITY_ATTEMPTS',
    'owner_change_cursor',
  ],
  files.provider,
);
requireMarkers(
  owner,
  [
    'pub(crate) async fn apply_exact_translation_in_tx(',
    'target locale revision does not match the translation proposal',
    '.filter(taxonomy_term_translation::Column::Revision.eq(target.revision))',
    '.filter(taxonomy_term::Column::Revision.eq(term.revision))',
    'taxonomy term changed before the localized update could commit',
  ],
  files.owner,
);
requireMarkers(
  changeWriter,
  ['id: Set(generate_id())', 'operation == "delete"', '"deleted"'],
  files.changeWriter,
);
requireMarkers(
  normalizedPlan,
  [
    'translation_target_postgres.rs',
    'RUSTOK_TAXONOMY_TEST_DATABASE_URL',
    'canonical server Migrator',
    'Exactly one stale-revision candidate may commit',
    'hard deletion',
    'Final exact-head pull-request run `33407161450`',
    'Post-merge main run `33420044854`',
    'Result 4 is complete for the current runtime input fingerprints.',
    'runtime input fingerprints',
  ],
  files.plan,
);

if (failures.length > 0) {
  console.error('[verify-taxonomy-translation-target-postgres] FAIL');
  for (const failure of failures) {
    console.error(`- ${failure}`);
    if (process.env.GITHUB_ACTIONS === 'true') {
      console.error(`::error title=Taxonomy translation evidence::${workflowCommandValue(failure)}`);
    }
  }
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-taxonomy-translation-target-postgres] PASS source=canonical-migrator+harness+owner+provider+workflow runtime=exact-head+post-merge-recorded+fingerprinted result4=complete',
);
