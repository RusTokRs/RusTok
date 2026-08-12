#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve('.');
const failures = [];

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-duplicate-delivery-race.json';
const harnessPath =
  'crates/rustok-blog/tests/comment_projection_duplicate_race_postgres_test.rs';
const harnessCommand =
  'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_duplicate_race_postgres_test concurrent_duplicate_envelope_commits_once_and_replays_cleanly -- --exact';

function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return '';
  }
  return readFileSync(target, 'utf8');
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireNoMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const harness = read(harnessPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'const WORKER_A_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_a";',
  'const WORKER_B_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_b";',
  'async fn concurrent_duplicate_envelope_commits_once_and_replays_cleanly()',
  'SELECT id FROM blog_posts WHERE tenant_id = $1 AND id = $2 FOR UPDATE',
  'let task_a = spawn_projection(worker_a, Arc::clone(&start), envelope.clone());',
  'let task_b = spawn_projection(worker_b, Arc::clone(&start), envelope.clone());',
  'wait_for_both_workers_to_block(&test_db.control).await?;',
  'lock_txn.commit().await?;',
  'success_count, 1,',
  'assert_eq!(\n        failure_count, 1',
  'load_post_state(&test_db.db, tenant_id, post_id).await?',
  'count_delivery(&test_db.db, envelope.id).await?, 1',
  'count_outbox_events(&test_db.db).await?, 1',
  'BlogCommentProjectionHandler::new(replay_db)',
  '.handle(&envelope)',
  'FROM pg_stat_activity',
  "application_name IN ('{WORKER_A_APPLICATION_NAME}', '{WORKER_B_APPLICATION_NAME}')",
  "wait_event_type = 'Lock'",
  'if blocked == 2',
  'SELECT set_config(\'application_name\', $1, false)',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
  '.max_connections(1)',
  'SET search_path TO "{schema_name}"',
]) {
  requireMarker(harness, marker, harnessPath);
}
requireNoMarker(harness, '#[ignore]', harnessPath);
requireNoMarker(harness, 'runtime_verified', harnessPath);
requireNoMarker(harness, 'tokio::time::sleep(Duration::from_secs(', harnessPath);
requireNoMarker(harness, 'SET search_path TO "{schema_name}", public', harnessPath);

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_duplicate_delivery_race' ||
    evidence.owner !== 'rustok-blog' ||
    evidence.provider !== 'rustok-comments'
  ) {
    failures.push(`${evidencePath}: identity drift`);
  }
  if (evidence.status !== 'source_verified_no_compile') {
    failures.push(`${evidencePath}: status drift`);
  }
  if (evidence.compile_policy !== 'not_run_by_request') {
    failures.push(`${evidencePath}: compile policy drift`);
  }
  if (evidence.runtime_status !== 'pending') {
    failures.push(`${evidencePath}: runtime status drift`);
  }

  const retained = evidence.harness ?? {};
  if (
    retained.status !== 'executable_no_run' ||
    retained.runtime_status !== 'not_run' ||
    retained.path !== harnessPath ||
    retained.environment !== 'RUSTOK_BLOG_TEST_DATABASE_URL' ||
    retained.command !== harnessCommand ||
    retained.isolation !==
      'unique_schema_three_independent_connections_two_named_workers_controlled_row_lock' ||
    retained.scope !==
      'same_envelope_concurrent_delivery_unique_ledger_loser_rollback_and_clean_replay' ||
    retained.non_claim !==
      'does_not_record_postgresql_execution_or_full_event_dispatcher_delivery'
  ) {
    failures.push(`${evidencePath}: harness metadata drift`);
  }

  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    'both_workers_pass_initial_delivery_lookup',
    'single_duplicate_commit',
    'losing_transaction_rolls_back',
    'same_envelope_replay_is_clean',
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

if (failures.length > 0) {
  console.error('Blog duplicate delivery race verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  'Blog concurrent duplicate delivery lock, rollback, cardinality, and replay source contract is consistent',
);
