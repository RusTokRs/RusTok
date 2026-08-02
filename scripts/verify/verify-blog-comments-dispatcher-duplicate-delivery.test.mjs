#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const verifier = path.resolve(
  'scripts/verify/verify-blog-comments-dispatcher-duplicate-delivery.mjs',
);

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function runFixture({
  missingSecondPublish = false,
  missingObservationCounter = false,
  missingFailureObservation = false,
  missingSingleCommit = false,
  missingEvidenceCase = false,
  statusDrift = false,
  runtimeStatusDrift = false,
  publicSearchPathFallback = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-dispatcher-duplicate-'));
  const evidencePath =
    'crates/rustok-blog/contracts/evidence/blog-comments-dispatcher-duplicate-delivery.json';
  const harnessPath =
    'crates/rustok-blog/tests/comment_projection_dispatcher_duplicate_postgres_test.rs';
  const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
  const harnessCommand =
    'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test event_dispatcher_replays_duplicate_envelope_without_double_commit -- --exact';

  write(
    root,
    harnessPath,
    `
use async_trait::async_trait;
const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";
const DISPATCHER_DUPLICATE_DELIVERIES: usize = 2;
struct ObservedProjectionHandler
inner: Arc<dyn EventHandler>
completed: Arc<AtomicUsize>
failed: Arc<AtomicUsize>
impl EventHandler for ObservedProjectionHandler
self.inner.handles(event)
let result = self.inner.handle(envelope).await;
if result.is_err()
${missingFailureObservation ? '' : 'self.failed.fetch_add(1, Ordering::SeqCst);'}
${missingObservationCounter ? '' : 'self.completed.fetch_add(1, Ordering::SeqCst);'}
async fn event_dispatcher_replays_duplicate_envelope_without_double_commit()
BlogModule.register_event_listeners(&mut registry, &context);
let mut handlers = registry.into_handlers();
assert_eq!(handlers.len(), 1);
assert_eq!(projection.name(), "blog_comment_projection");
let failed = Arc::new(AtomicUsize::new(0));
Arc::clone(&failed)
let mut dispatcher = EventDispatcher::with_config(
fail_fast: true
max_concurrent: 1
retry_count: 0
dispatcher.register(observed);
${missingSecondPublish ? '' : 'for _ in 0..DISPATCHER_DUPLICATE_DELIVERIES'}
running.bus().publish_envelope(envelope.clone())?;
wait_for_completed_dispatches(&completed).await?;
completed.load(Ordering::SeqCst)
DISPATCHER_DUPLICATE_DELIVERIES
assert_eq!(failed.load(Ordering::SeqCst), 0);
load_post_state(&test_db.db, tenant_id, post_id).await?, (1, 2)
count_delivery(&test_db.db, envelope.id).await?, 1
${missingSingleCommit ? '' : 'count_outbox_events(&test_db.db).await?, 1'}
async fn wait_for_completed_dispatches(completed: &AtomicUsize)
event dispatcher did not complete both duplicate deliveries
CREATE TABLE blog_comment_projection_deliveries
CREATE TABLE sys_events
.max_connections(1)
SET search_path TO "{schema_name}"${publicSearchPathFallback ? ', public' : ''}
`,
  );

  const cases = [
    {
      name: 'module_registered_handler_routed_twice',
      expected: 'same envelope reaches the module-registered handler twice',
    },
    {
      name: 'two_completed_dispatch_calls',
      expected: 'two handler calls complete with zero errors',
    },
    ...(
      missingEvidenceCase
        ? []
        : [
            {
              name: 'single_transactional_application',
              expected: 'one counter, delivery, and outbox application remains',
            },
          ]
    ),
  ];

  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'comments_dispatcher_duplicate_delivery',
      status: statusDrift ? 'runtime_verified' : 'source_verified_no_compile',
      compile_policy: 'not_run_by_request',
      runtime_status: runtimeStatusDrift ? 'verified' : 'pending',
      owner: 'rustok-blog',
      provider: 'rustok-comments',
      harness: {
        status: 'executable_no_run',
        runtime_status: 'not_run',
        path: harnessPath,
        environment: 'RUSTOK_BLOG_TEST_DATABASE_URL',
        command: harnessCommand,
        isolation:
          'unique_schema_one_connection_pool_module_registered_handler_observed_two_dispatches',
        scope:
          'same_envelope_published_twice_through_event_bus_dispatcher_single_projection_commit',
        non_claim: 'does_not_record_postgresql_execution_or_concurrent_duplicate_race',
      },
      cases,
    }),
  );

  write(
    root,
    planPath,
    `
blog-comments-dispatcher-duplicate-delivery.json
comment_projection_dispatcher_duplicate_postgres_test
event_dispatcher_replays_duplicate_envelope_without_double_commit
${harnessCommand}
dispatcher-level duplicate delivery
source_verified_no_compile
Slice 56
`,
  );

  const result = spawnSync(process.execPath, [verifier], {
    cwd: root,
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: 'utf8',
  });
  rmSync(root, { recursive: true, force: true });
  return result;
}

test('Blog dispatcher duplicate verifier accepts the canonical source contract', () => {
  const result = runFixture();
  assert.equal(result.status, 0, result.stderr);
});

test('Blog dispatcher duplicate verifier rejects removal of the two-delivery loop', () => {
  const result = runFixture({ missingSecondPublish: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /for _ in 0\.\.DISPATCHER_DUPLICATE_DELIVERIES/);
});

test('Blog dispatcher duplicate verifier rejects removal of completed-call observation', () => {
  const result = runFixture({ missingObservationCounter: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /self\.completed\.fetch_add/);
});

test('Blog dispatcher duplicate verifier rejects removal of handler-error observation', () => {
  const result = runFixture({ missingFailureObservation: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /self\.failed\.fetch_add/);
});

test('Blog dispatcher duplicate verifier rejects removal of the single-commit assertion', () => {
  const result = runFixture({ missingSingleCommit: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /count_outbox_events/);
});

test('Blog dispatcher duplicate verifier rejects fallback to public tables', () => {
  const result = runFixture({ publicSearchPathFallback: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /forbidden SET search_path TO "\{schema_name\}", public/);
});

test('Blog dispatcher duplicate verifier rejects missing evidence coverage', () => {
  const result = runFixture({ missingEvidenceCase: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /missing case single_transactional_application/);
});

test('Blog dispatcher duplicate verifier rejects false runtime promotion', () => {
  const result = runFixture({ statusDrift: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /status drift/);
});

test('Blog dispatcher duplicate verifier rejects runtime-status promotion', () => {
  const result = runFixture({ runtimeStatusDrift: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /runtime status drift/);
});
