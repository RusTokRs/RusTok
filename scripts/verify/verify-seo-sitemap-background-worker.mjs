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
const worker = read('crates/rustok-seo/src/services/sitemap_background.rs');
const graphql = read('crates/rustok-seo/src/graphql/mod.rs');

requireText(
  bulkModule,
  'include!("sitemap_background.rs");',
  'background sitemap include',
);
requireText(
  applications,
  'self.runtime.queue_sitemap_generation_background(tenant).await',
  'request-path enqueue routing',
);
requireText(
  applications,
  'pub async fn execute_next_sitemap_job(&self)',
  'worker application boundary',
);
requireText(
  applications,
  'self.runtime.execute_next_sitemap_job_background().await',
  'worker runtime routing',
);
for (const [value, label] of [
  ['const SITEMAP_JOB_QUEUED: &str = "queued";', 'queued phase'],
  ['const SITEMAP_JOB_RUNNING: &str = "running";', 'generation phase'],
  ['const SITEMAP_JOB_SUBMITTING: &str = "submitting";', 'submission phase'],
  ['pub(super) async fn queue_sitemap_generation_background(', 'queue implementation'],
  ['pub(super) async fn execute_next_sitemap_job_background(', 'worker implementation'],
  ['async fn execute_sitemap_generation_phase(', 'generation worker phase'],
  ['async fn execute_sitemap_submission_phase(', 'submission worker phase'],
  ['async fn persist_background_sitemap_generation(', 'transactional generation persistence'],
  ['async fn record_background_sitemap_submission(', 'transactional submission persistence'],
  ['SeoSitemapGenerated', 'generated event'],
  ['SeoSitemapSubmitted', 'submitted event'],
  ['SITEMAP_JOB_SUBMITTING.to_string()', 'generation-to-submission checkpoint'],
  ['SITEMAP_JOB_COMPLETED.to_string()', 'terminal completion'],
  ['SITEMAP_JOB_RUNNING,\n                SITEMAP_JOB_SUBMITTING', 'active job resume'],
  ['active.is_some()', 'tenant-local queue deduplication'],
  ['urls.chunks(super::SITEMAP_CHUNK_SIZE)', 'bounded sitemap files'],
]) {
  requireText(worker, value, label);
}

forbidText(
  applications,
  'self.runtime.generate_sitemaps(tenant).await',
  'synchronous sitemap facade routing',
);
forbidText(
  graphql,
  'execute_next_sitemap_job',
  'worker execution from GraphQL request path',
);

const queueStart = worker.indexOf('pub(super) async fn queue_sitemap_generation_background(');
const workerStart = worker.indexOf('pub(super) async fn execute_next_sitemap_job_background(');
if (queueStart < 0 || workerStart < 0 || workerStart <= queueStart) {
  failures.push('unable to isolate sitemap queue implementation');
} else {
  const queueBody = worker.slice(queueStart, workerStart);
  for (const forbidden of [
    'collect_background_sitemap_urls(',
    'persist_background_sitemap_generation(',
    'submit_background_sitemap_endpoints(',
  ]) {
    forbidText(queueBody, forbidden, 'request-path sitemap work');
  }
}

if (failures.length > 0) {
  console.error('SEO sitemap background-worker verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ sitemap mutations only enqueue and one worker invocation executes one durable generation or submission phase',
);
