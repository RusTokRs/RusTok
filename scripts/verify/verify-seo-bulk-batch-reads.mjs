#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const applications = read('crates/rustok-seo/src/services/applications.rs');
const bulkReads = read('crates/rustok-seo/src/services/applications/bulk_reads.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(applications, 'mod bulk_reads;', 'application module routing');
requireText(
  applications,
  'self.runtime.list_bulk_items_batched(tenant, input).await',
  'focused bulk service routing',
);

for (const [value, label] of [
  ['const BULK_META_BATCH_SIZE: usize = 256;', 'bounded batch size'],
  ['fn load_bulk_explicit_meta_batches(', 'batch loader'],
  ['target_ids.chunks(BULK_META_BATCH_SIZE)', 'bounded target chunks'],
  ['seo_meta::Column::TargetId.is_in(', 'metadata batch predicate'],
  ['meta_translation::Column::MetaId.is_in(', 'translation batch predicate'],
  ['let settings = self.load_settings(tenant.id).await?;', 'single settings snapshot'],
  ['resolve_batched_bulk_meta(', 'batched projection'],
]) {
  requireText(bulkReads, value, label);
}

for (const [value, label] of [
  ['.seo_meta(', 'per-target metadata service call'],
  ['load_explicit_meta(', 'per-target explicit metadata call'],
  ['meta_translation::Column::MetaId.eq(', 'per-meta translation query'],
]) {
  forbidText(bulkReads, value, label);
}

const settingsLoads = bulkReads.match(/load_settings\(tenant\.id\)/g) ?? [];
if (settingsLoads.length !== 1) {
  failures.push(`expected one settings load, found ${settingsLoads.length}`);
}

if (failures.length > 0) {
  console.error('SEO bulk batch-read verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ SEO bulk list uses bounded metadata and translation batch reads');
