#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const applications = read('crates/rustok-seo/src/services/applications.rs');
const bulkModule = read('crates/rustok-seo/src/services/bulk.rs');
const compatibilityPoller = read(
  'crates/rustok-seo/src/services/bulk_io_bounded_compat.rs',
);
const worker = read('crates/rustok-seo/src/services/index_repair_background.rs');
const migrations = read('crates/rustok-seo/src/migrations/mod.rs');
const migration = read(
  'crates/rustok-seo/src/migrations/m20260724_000008_create_seo_index_repair_jobs.rs',
);
const graphql = read('crates/rustok-seo/src/graphql/mod.rs');
const hostLifecycle = read('apps/server/src/services/app_lifecycle.rs');

requireText(
  bulkModule,
  'include!("index_repair_background.rs");',
  'background index repair include',
);
requireText(
  applications,
  '.queue_index_repair_replay_background(',
  'request-path enqueue routing',
);
requireText(
  applications,
  'pub async fn execute_next_index_repair_replay_job(',
  'worker application boundary',
);
requireText(
  applications,
  '_authorization: &SeoWorkerAuthorization',
  'worker authorization grant',
);
requireText(
  applications,
  '.execute_next_index_repair_replay_job_background()',
  'worker runtime routing',
);
for (const [value, label] of [
  ['const INDEX_REPAIR_JOB_QUEUED: &str = "queued";', 'queued state'],
  ['const INDEX_REPAIR_JOB_RUNNING: &str = "running";', 'running state'],
  ['const INDEX_REPAIR_JOB_COMPLETED: &str = "completed";', 'completed state'],
  ['const INDEX_REPAIR_JOB_FAILED: &str = "failed";', 'failed state'],
  ['const INDEX_REPAIR_JOB_MAX_LIMIT: usize = 500;', 'bounded worker limit'],
  ['pub(super) async fn queue_index_repair_replay_background(', 'queue implementation'],
  ['pub(super) async fn execute_next_index_repair_replay_job_background(', 'worker implementation'],
  ['job_entity::Column::Status.eq(INDEX_REPAIR_JOB_RUNNING)', 'running job resume'],
  ['job_entity::Column::Status.eq(INDEX_REPAIR_JOB_QUEUED)', 'queued job claim'],
  ['.run_index_repair_replay(', 'bounded legacy execution inside worker'],
  ['active.status = Set(INDEX_REPAIR_JOB_COMPLETED.to_string());', 'terminal checkpoint'],
  ['fail_background_index_repair_job', 'durable failure checkpoint'],
]) {
  requireText(worker, value, label);
}
for (const [value, label] of [
  ['execute_next_bulk_job_only_with_bounded_io()', 'bounded bulk poll'],
  ['execute_next_sitemap_job_background()', 'sitemap queue poll'],
  ['execute_next_index_repair_replay_job_background()', 'index queue poll'],
]) {
  requireText(compatibilityPoller, value, label);
}
for (const [value, label] of [
  ['SeoWorkerAuthorization::from_runtime_config(', 'host worker authorization'],
  ['settings.runtime.runs_background_workers()', 'host-mode authorization input'],
  ['seo_bulk_worker_enabled', 'SEO worker switch authorization input'],
  ['.execute_next_bulk_job(&authorization)', 'authorized server SEO poller lifecycle'],
]) {
  requireText(hostLifecycle, value, label);
}
requireText(
  migrations,
  'mod m20260724_000008_create_seo_index_repair_jobs;',
  'migration module registration',
);
requireText(
  migration,
  '.table(SeoIndexRepairJobs::Table)',
  'durable job table',
);
requireText(
  migration,
  'idx_seo_index_repair_jobs_status_created',
  'worker claim index',
);
forbidText(
  applications,
  '.run_index_repair_replay(tenant_id, target_type, limit, replay_historical)',
  'synchronous application routing',
);
forbidText(
  graphql,
  'execute_next_index_repair_replay_job',
  'worker execution from GraphQL request path',
);
forbidText(
  hostLifecycle,
  'service.bulk().execute_next_bulk_job().await',
  'unauthorized server worker execution',
);

const queueStart = worker.indexOf('pub(super) async fn queue_index_repair_replay_background(');
const workerStart = worker.indexOf(
  'pub(super) async fn execute_next_index_repair_replay_job_background(',
);
if (queueStart < 0 || workerStart < 0 || workerStart <= queueStart) {
  failures.push('unable to isolate index repair queue implementation');
} else {
  const queueBody = worker.slice(queueStart, workerStart);
  forbidText(queueBody, '.run_index_repair_replay(', 'request-path repair/replay execution');
}

if (failures.length > 0) {
  console.error('SEO index repair background-worker verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ index repair/replay requests enqueue durable jobs and the explicitly authorized server SEO poller advances all durable queues',
);
