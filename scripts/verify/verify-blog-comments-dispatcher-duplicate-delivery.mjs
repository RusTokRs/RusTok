#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve('.');
const failures = [];

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-dispatcher-duplicate-delivery.json';
const harnessPath =
  'crates/rustok-blog/tests/comment_projection_dispatcher_duplicate_postgres_test.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan.md';
const harnessCommand =
  'RUSTOK_BLOG_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test event_dispatcher_replays_duplicate_envelope_without_double_commit -- --exact';

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
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  'use async_trait::async_trait;',
  'const BLOG_TEST_DATABASE_ENV: &str = "RUSTOK_BLOG_TEST_DATABASE_URL";',
  'const DISPATCHER_DUPLICATE_DELIVERIES: usize = 2;',
  'struct ObservedProjectionHandler',
  'inner: Arc<dyn EventHandler>',
  'completed: Arc<AtomicUsize>',
  'failed: Arc<AtomicUsize>',
  'impl EventHandler for ObservedProjectionHandler',
  'self.inner.handles(event)',
  'let result = self.inner.handle(envelope).await;',
  'if result.is_err()',
  'self.failed.fetch_add(1, Ordering::SeqCst);',
  'self.completed.fetch_add(1, Ordering::SeqCst);',
  'async fn event_dispatcher_replays_duplicate_envelope_without_double_commit()',
  'BlogModule.register_event_listeners(&mut registry, &context);',
  'let mut handlers = registry.into_handlers();',
  'assert_eq!(handlers.len(), 1);',
  'assert_eq!(projection.name(), "blog_comment_projection");',
  'let failed = Arc::new(AtomicUsize::new(0));',
  'Arc::clone(&failed)',
  'let mut dispatcher = EventDispatcher::with_config(',
  'fail_fast: true',
  'max_concurrent: 1',
  'retry_count: 0',
  'dispatcher.register(observed);',
  'for _ in 0..DISPATCHER_DUPLICATE_DELIVERIES',
  'running.bus().publish_envelope(envelope.clone())?;',
  'wait_for_completed_dispatches(&completed).await?;',
  'completed.load(Ordering::SeqCst)',
  'DISPATCHER_DUPLICATE_DELIVERIES',
  'assert_eq!(failed.load(Ordering::SeqCst), 0);',
  'load_post_state(&test_db.db, tenant_id, post_id).await?',
  'count_delivery(&test_db.db, envelope.id).await?, 1',
  'count_outbox_events(&test_db.db).await?, 1',
  'async fn wait_for_completed_dispatches(completed: &AtomicUsize)',
  'event dispatcher did not complete both duplicate deliveries',
  'CREATE TABLE blog_comment_projection_deliveries',
  'CREATE TABLE sys_events',
  '.max_connections(1)',
  'SET search_path TO "{schema_name}"',
]) {
  requireMarker(harness, marker, harnessPath);
}
requireNoMarker(harness, '#[ignore]', harnessPath);
requireNoMarker(harness, 'runtime_verified', harnessPath);
requireNoMarker(harness, 'SET search_path TO "{schema_name}", public', harnessPath);

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== 'blog' ||
    evidence.surface !== 'comments_dispatcher_duplicate_delivery' ||
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
      'unique_schema_one_connection_pool_module_registered_handler_observed_two_dispatches' ||
    retained.scope !==
      'same_envelope_published_twice_through_event_bus_dispatcher_single_projection_commit' ||
    retained.non_claim !==
      'does_not_record_postgresql_execution_or_concurrent_duplicate_race'
  ) {
    failures.push(`${evidencePath}: harness metadata drift`);
  }

  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    'module_registered_handler_routed_twice',
    'two_completed_dispatch_calls',
    'single_transactional_application',
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

for (const marker of [
  'blog-comments-dispatcher-duplicate-delivery.json',
  'comment_projection_dispatcher_duplicate_postgres_test',
  'event_dispatcher_replays_duplicate_envelope_without_double_commit',
  harnessCommand,
  'dispatcher-level duplicate delivery',
  'source_verified_no_compile',
  'Slice 56',
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error('Blog dispatcher duplicate delivery verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  'Blog dispatcher duplicate delivery routing, successful acknowledgement, observation, and single-commit source contract is consistent',
);
