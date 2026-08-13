#!/usr/bin/env node

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

function verifyRecordedRuntimeEvidence(evidence, label) {
  const runtime = evidence.runtime_evidence ?? {};
  const exact = runtime.exact_head_pull_request ?? {};
  const postMerge = runtime.post_merge_main ?? {};

  if (
    runtime.workflow !== 'Taxonomy PostgreSQL Evidence' ||
    runtime.required_backend !== 'PostgreSQL 16'
  ) {
    failures.push(`${label}: runtime evidence environment drift`);
  }

  if (
    exact.run_id !== 31738994542 ||
    exact.head_sha !== '2cde81ad6bbf7b544e09fd68c2374488f587593e' ||
    exact.runtime_job_id !== 94577622139 ||
    exact.gate_job_id !== 94579422423 ||
    exact.conclusion !== 'success' ||
    exact.artifact_id !== 9196489480 ||
    exact.artifact_digest !==
      'sha256:cb550e168911af07564d147b27cfcbad3557dd0ff86531b6317c0d3186c244e6'
  ) {
    failures.push(`${label}: exact-head runtime provenance drift`);
  }

  if (
    postMerge.run_id !== 31745429243 ||
    postMerge.head_sha !== '32b2255337bb090acef5a41ea4649a3a60e81110' ||
    postMerge.runtime_job_id !== 94598773113 ||
    postMerge.gate_job_id !== 94601290823 ||
    postMerge.conclusion !== 'success' ||
    postMerge.artifact_id !== 9199060002 ||
    postMerge.artifact_digest !==
      'sha256:2132b65d576c958504b11e6bcda36296f1f99f8fb314a8e3399ad974c0155d23'
  ) {
    failures.push(`${label}: post-merge runtime provenance drift`);
  }
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
    failures.push(`${files.evidence}: remaining Open result 4 evidence drift`);
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
    'runtime evidence',
    'Result 4 is complete',
  ],
  files.plan,
);

if (failures.length > 0) {
  console.error('[verify-taxonomy-translation-target-postgres] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-taxonomy-translation-target-postgres] PASS source=canonical-migrator+harness+owner+provider+workflow runtime=recorded',
);
