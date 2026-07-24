#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const wrapper = read('crates/rustok-seo/src/services/diagnostics.rs');
const legacy = read('crates/rustok-seo/src/services/diagnostics_legacy.rs');
const batch = read('crates/rustok-seo/src/services/diagnostics_batch.rs');

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

if (wrapper.trim() !== 'include!("diagnostics_batch.rs");') {
  failures.push('diagnostics wrapper must activate only the batched implementation');
}

const legacyBlobHeader = Buffer.from(`blob ${Buffer.byteLength(legacy)}\0`);
const legacyBlobSha = createHash('sha1')
  .update(legacyBlobHeader)
  .update(legacy)
  .digest('hex');
if (legacyBlobSha !== '394bc0321e3daa5473b0491b257fc4c17c830cc8') {
  failures.push(`legacy diagnostics blob changed: ${legacyBlobSha}`);
}

for (const [value, label] of [
  ['const DIAGNOSTICS_META_BATCH_SIZE: usize = 256;', 'bounded batch size'],
  ['target_ids.chunks(DIAGNOSTICS_META_BATCH_SIZE)', 'bounded target chunks'],
  ['seo_meta::Column::TargetId.is_in(', 'metadata batch predicate'],
  ['meta_translation::Column::MetaId.is_in(', 'translation batch predicate'],
  ['let settings = if enabled {', 'single settings snapshot'],
  ['load_target_state(', 'authoring target projection'],
  ['load_route_target_state(', 'public route projection'],
  ['build_diagnostics_meta_record(', 'metadata projection'],
  ['build_diagnostics_page_context(', 'page-context projection'],
]) {
  requireText(batch, value, label);
}

for (const [value, label] of [
  ['.seo_meta(', 'per-target metadata service call'],
  ['.resolve_page_context(', 'per-target route resolver call'],
  ['load_explicit_meta(', 'per-target explicit metadata call'],
  ['meta_translation::Column::MetaId.eq(', 'per-meta translation query'],
]) {
  forbidText(batch, value, label);
}

const settingsLoads = batch.match(/load_settings\(tenant\.id\)/g) ?? [];
if (settingsLoads.length !== 1) {
  failures.push(`expected one settings load, found ${settingsLoads.length}`);
}

const metadataBatchQueries = batch.match(/seo_meta::Entity::find\(\)/g) ?? [];
if (metadataBatchQueries.length !== 1) {
  failures.push(`expected one metadata batch query definition, found ${metadataBatchQueries.length}`);
}

const translationBatchQueries = batch.match(/meta_translation::Entity::find\(\)/g) ?? [];
if (translationBatchQueries.length !== 1) {
  failures.push(
    `expected one translation batch query definition, found ${translationBatchQueries.length}`,
  );
}

if (failures.length > 0) {
  console.error('SEO diagnostics batch-read verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO diagnostics uses bounded metadata batches and direct shared-snapshot projections',
);
