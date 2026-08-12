#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve(
  'scripts/verify/verify-blog-comments-duplicate-delivery-race.mjs',
);

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingBlockedWorkerObservation = false,
  missingReplay = false,
  statusDrift = false,
  publicSearchPathFallback = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-duplicate-race-'));
  const evidencePath =
    'crates/rustok-blog/contracts/evidence/blog-comments-duplicate-delivery-race.json';
  const harnessPath =
    'crates/rustok-blog/tests/comment_projection_duplicate_race_postgres_test.rs';
  const command =
    'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_duplicate_race_postgres_test concurrent_duplicate_envelope_commits_once_and_replays_cleanly -- --exact';

  write(
    root,
    harnessPath,
    `
const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const WORKER_A_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_a";
const WORKER_B_APPLICATION_NAME: &str = "rustok_blog_duplicate_race_b";
async fn concurrent_duplicate_envelope_commits_once_and_replays_cleanly()
SELECT id FROM blog_posts WHERE tenant_id = $1 AND id = $2 FOR UPDATE
let task_a = spawn_projection(worker_a, Arc::clone(&start), envelope.clone());
let task_b = spawn_projection(worker_b, Arc::clone(&start), envelope.clone());
${missingBlockedWorkerObservation ? '' : 'wait_for_both_workers_to_block(&test_db.control).await?;'}
lock_txn.commit().await?;
success_count, 1,
assert_eq!(
        failure_count, 1
load_post_state(&test_db.db, tenant_id, post_id).await?, (1, 2)
count_delivery(&test_db.db, envelope.id).await?, 1
count_outbox_events(&test_db.db).await?, 1
${missingReplay ? '' : `BlogCommentProjectionHandler::new(replay_db)
.handle(&envelope)`}
FROM pg_stat_activity
application_name IN ('{WORKER_A_APPLICATION_NAME}', '{WORKER_B_APPLICATION_NAME}')
wait_event_type = 'Lock'
if blocked == 2
SELECT set_config('application_name', $1, false)
CREATE TABLE blog_comment_projection_deliveries
CREATE TABLE sys_events
.max_connections(1)
SET search_path TO "{schema_name}"${publicSearchPathFallback ? ', public' : ''}
`,
  );

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_duplicate_delivery_race',
      status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: 'pending',
      owner: 'rustok-blog',
      provider: 'rustok-comments',
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        path: harnessPath,
        environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
        command,
        isolation:
          'unique_schema_three_independent_connections_two_named_workers_controlled_row_lock',
        scope:
          'same_envelope_concurrent_delivery_unique_ledger_loser_rollback_and_clean_replay',
        non_claim:
          'does_not_record_postgresql_execution_or_full_event_dispatcher_delivery',
      },
      cases: [
        { name: 'both_workers_pass_initial_delivery_lookup' },
        { name: 'single_duplicate_commit' },
        { name: 'losing_transaction_rolls_back' },
        { name: 'same_envelope_replay_is_clean' },
      ],
    }),
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve('.'),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
}

function expectRejected(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('accepts the canonical duplicate delivery race contract', () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a race without blocked-worker observation', () => {
  expectRejected(
    { missingBlockedWorkerObservation: true },
    /missing wait_for_both_workers_to_block/,
  );
});

test('rejects a race without clean same-envelope replay', () => {
  expectRejected({ missingReplay: true }, /missing BlogCommentProjectionHandler::new/);
});

test('rejects a race harness that can fall back to public tables', () => {
  expectRejected(
    { publicSearchPathFallback: true },
    /forbidden SET search_path TO "\{schema_name\}", public/,
  );
});

test('rejects runtime promotion without execution', () => {
  expectRejected({ statusDrift: true }, /status drift/);
});
