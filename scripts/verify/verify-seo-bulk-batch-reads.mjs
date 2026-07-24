#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const exists = (relativePath) => existsSync(fileURLToPath(new URL(relativePath, root)));

const applications = read('crates/rustok-seo/src/services/applications.rs');
const services = read('crates/rustok-seo/src/services/mod.rs');
const bulkModule = read('crates/rustok-seo/src/services/bulk.rs');
const legacy = read('crates/rustok-seo/src/services/bulk_legacy.rs');
const readModel = read('crates/rustok-seo/src/services/bulk_read_model.rs');
const execution = read('crates/rustok-seo/src/services/bulk_batch_execution.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(services, 'mod bulk_read_model;', 'shared read-model registration');
requireText(bulkModule, 'include!("bulk_legacy.rs");', 'legacy bulk include');
requireText(bulkModule, 'include!("bulk_batch_execution.rs");', 'batched execution include');
requireText(legacy, 'pub async fn execute_next_bulk_job(', 'legacy implementation preservation');
if (exists('crates/rustok-seo/src/services/applications/bulk_reads.rs')) {
  failures.push('application-local bulk reader must be removed after shared extraction');
}

for (const [value, label] of [
  ['self.runtime.list_bulk_items_batched(tenant, input).await', 'bulk list routing'],
  ['.preview_bulk_selection_count_batched(tenant, selection)', 'selection preview routing'],
  ['.queue_bulk_apply_batched(tenant, created_by, input)', 'apply queue routing'],
  ['.queue_bulk_export_batched(tenant, created_by, input)', 'export queue routing'],
  ['self.runtime.execute_next_bulk_job_batched().await', 'worker routing'],
]) {
  requireText(applications, value, label);
}

for (const [value, label] of [
  ['const BULK_META_BATCH_SIZE: usize = 256;', 'bounded batch size'],
  ['pub(super) async fn collect_bulk_read_rows(', 'shared batch collector'],
  ['target_ids.chunks(BULK_META_BATCH_SIZE)', 'bounded target chunks'],
  ['seo_meta::Column::TargetId.is_in(', 'metadata batch predicate'],
  ['meta_translation::Column::MetaId.is_in(', 'translation batch predicate'],
  ['let settings = self.load_settings(tenant.id).await?;', 'single settings snapshot'],
  ['fn resolve_bulk_read_projection(', 'shared full projection'],
  ['pub structured_data: Option<Value>', 'export structured-data projection'],
]) {
  requireText(readModel, value, label);
}

for (const [value, label] of [
  ['pub(super) async fn preview_bulk_selection_count_batched(', 'batched preview count'],
  ['pub(super) async fn queue_bulk_apply_batched(', 'batched apply queue'],
  ['pub(super) async fn queue_bulk_export_batched(', 'batched export queue'],
  ['pub(super) async fn execute_next_bulk_job_batched(', 'batched worker'],
  ['self.collect_bulk_read_rows(', 'shared collector reuse'],
  ['self.execute_apply_job_batched(&running).await', 'batched apply execution'],
  ['self.execute_export_job_batched(&running).await', 'batched export execution'],
  ['fn export_bulk_projection_row(', 'projection CSV serializer'],
]) {
  requireText(execution, value, label);
}

for (const [source, value, label] of [
  [readModel, '.seo_meta(', 'read-model per-target metadata service call'],
  [readModel, 'load_explicit_meta(', 'read-model per-target explicit query'],
  [readModel, 'meta_translation::Column::MetaId.eq(', 'read-model per-meta translation query'],
  [execution, '.seo_meta(', 'worker per-target metadata service call'],
  [applications, '.preview_bulk_selection_count(tenant, selection)', 'legacy selection routing'],
  [applications, '.queue_bulk_export(tenant, created_by, input)', 'legacy export routing'],
  [applications, 'self.runtime.execute_next_bulk_job().await', 'legacy worker routing'],
]) {
  forbidText(source, value, label);
}

const settingsLoads = readModel.match(/load_settings\(tenant\.id\)/g) ?? [];
if (settingsLoads.length !== 1) {
  failures.push(`expected one shared settings load, found ${settingsLoads.length}`);
}

if (failures.length > 0) {
  console.error('SEO bulk batch-read verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO bulk list, selection, preview, and export use the shared bounded metadata read model',
);
