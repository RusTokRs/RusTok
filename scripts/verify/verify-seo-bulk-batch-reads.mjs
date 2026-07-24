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
const applyExecution = read('crates/rustok-seo/src/services/bulk_bounded_execution.rs');
const ioExecution = read('crates/rustok-seo/src/services/bulk_io_bounded_execution.rs');
const ioCompatibility = read('crates/rustok-seo/src/services/bulk_io_bounded_compat.rs');
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

requireText(services, 'mod bulk_read_model;', 'shared read-model registration');
requireText(bulkModule, 'include!("bulk_legacy.rs");', 'legacy bulk include');
requireText(bulkModule, 'include!("bulk_bounded_execution.rs");', 'bounded apply include');
requireText(bulkModule, 'include!("bulk_io_bounded_execution.rs");', 'bounded IO include');
requireText(bulkModule, 'include!("bulk_io_bounded_compat.rs");', 'bounded IO compatibility include');
requireText(legacy, 'pub async fn execute_next_bulk_job(', 'legacy implementation preservation');
if (exists('crates/rustok-seo/src/services/applications/bulk_reads.rs')) {
  failures.push('application-local bulk reader must be removed after shared extraction');
}
if (exists('crates/rustok-seo/src/services/bulk_batch_execution.rs')) {
  failures.push('superseded unbounded batch execution file must be removed');
}

for (const [value, label] of [
  ['self.runtime.list_bulk_items_batched(tenant, input).await', 'bulk list routing'],
  ['.preview_bulk_selection_count_batched(tenant, selection)', 'selection preview routing'],
  ['.queue_bulk_apply_batched(tenant, created_by, input)', 'apply queue routing'],
  ['.queue_bulk_export_bounded_io(tenant, created_by, input)', 'bounded export queue routing'],
  ['.queue_bulk_import_bounded_io(tenant, created_by, input)', 'bounded import queue routing'],
  ['self.runtime.execute_next_bulk_job_with_bounded_io().await', 'normalized bounded worker routing'],
]) {
  requireText(applications, value, label);
}

for (const [value, label] of [
  ['const BULK_META_BATCH_SIZE: usize = 256;', 'bounded metadata batch size'],
  ['pub(super) async fn collect_bulk_read_rows(', 'shared batch collector'],
  ['target_ids.chunks(BULK_META_BATCH_SIZE)', 'bounded metadata target chunks'],
  ['seo_meta::Column::TargetId.is_in(', 'metadata batch predicate'],
  ['meta_translation::Column::MetaId.is_in(', 'translation batch predicate'],
  ['let settings = self.load_settings(tenant.id).await?;', 'single settings snapshot'],
  ['fn resolve_bulk_read_projection(', 'shared full projection'],
  ['pub structured_data: Option<Value>', 'export structured-data projection'],
]) {
  requireText(readModel, value, label);
}

for (const [value, label] of [
  ['const BULK_APPLY_CHUNK_SIZE: usize = 50;', 'bounded apply chunk size'],
  ['struct QueuedBulkApplyPayload', 'persisted apply snapshot payload'],
  ['.take(BULK_APPLY_CHUNK_SIZE)', 'bounded apply worker slice'],
  ['self.execute_apply_job_chunk(&running).await', 'chunked apply execution'],
  ['async fn checkpoint_bulk_apply_job(', 'persisted apply progress checkpoint'],
  ['async fn load_bulk_job_progress(', 'item-derived progress recovery'],
]) {
  requireText(applyExecution, value, label);
}

for (const [value, label] of [
  ['const BULK_IO_CHUNK_SIZE: usize = 50;', 'bounded IO chunk size'],
  ['struct QueuedBulkExportPayload', 'persisted export target snapshot'],
  ['struct QueuedBulkImportPayload', 'persisted import cursor payload'],
  ['target_ids: Vec<Uuid>', 'export target IDs'],
  ['next_byte_offset: usize', 'import byte cursor'],
  ['next_row_number: usize', 'import row cursor'],
  ['pub(super) async fn queue_bulk_export_bounded_io(', 'bounded export queue'],
  ['pub(super) async fn queue_bulk_import_bounded_io(', 'bounded import queue'],
  ['.take(BULK_IO_CHUNK_SIZE)', 'bounded export target slice'],
  ['while rows.len() < BULK_IO_CHUNK_SIZE', 'bounded import row slice'],
  ['self.execute_export_job_chunk(job).await', 'chunked export execution'],
  ['self.execute_import_job_chunk(&normalized).await', 'chunked normalized import execution'],
  ['async fn checkpoint_bulk_io_job(', 'persisted IO progress checkpoint'],
  ['reader.position().byte()', 'streaming import byte cursor'],
  ['load_bulk_io_explicit_meta_batches(', 'bounded export metadata loader'],
  ['target_ids.chunks(BULK_IO_META_BATCH_SIZE)', 'bounded export metadata chunks'],
  ['meta_translation::Column::MetaId.is_in(', 'bounded export translation predicate'],
  ['resolve_bulk_io_projection(', 'bounded export projection'],
  ['seo-bulk-export-failures-', 'per-chunk export failure artifact'],
  ['seo-bulk-import-failures-', 'per-chunk import failure artifact'],
]) {
  requireText(ioExecution + ioCompatibility, value, label);
}

for (const [value, label] of [
  ['pub(super) async fn execute_next_bulk_job_with_bounded_io(', 'bounded IO worker entry point'],
  ['execute_export_job_chunk_compat(', 'empty export compatibility wrapper'],
  ['progress.artifacts == 0', 'single empty export artifact'],
  ['failed to write empty export CSV header', 'empty export header preservation'],
  ['normalize_bulk_import_job_payload(', 'import locale normalization'],
  ['payload.input.locale = job.locale.clone();', 'persisted normalized import locale'],
  ['serde_json::from_value::<QueuedBulkImportPayload>', 'legacy/new import payload compatibility'],
]) {
  requireText(ioCompatibility, value, label);
}

for (const [source, value, label] of [
  [readModel, '.seo_meta(', 'read-model per-target metadata service call'],
  [readModel, 'load_explicit_meta(', 'read-model per-target explicit query'],
  [readModel, 'meta_translation::Column::MetaId.eq(', 'read-model per-meta translation query'],
  [ioExecution, '.seo_meta(', 'IO worker per-target metadata service call'],
  [applications, '.queue_bulk_export_batched(tenant, created_by, input)', 'superseded export queue routing'],
  [applications, '.queue_bulk_import(tenant, created_by, input)', 'legacy import queue routing'],
  [applications, 'self.runtime.execute_next_bulk_job_fully_bounded().await', 'pre-normalization IO worker routing'],
  [applications, 'self.runtime.execute_next_bulk_job_batched().await', 'apply-only worker routing'],
  [applications, 'self.runtime.execute_next_bulk_job().await', 'legacy worker routing'],
]) {
  forbidText(source, value, label);
}

const settingsLoads = readModel.match(/load_settings\(tenant\.id\)/g) ?? [];
if (settingsLoads.length !== 1) {
  failures.push(`expected one shared settings load, found ${settingsLoads.length}`);
}

if (failures.length > 0) {
  console.error('SEO bulk bounded-worker verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO bulk reads stay batched and apply/export/import workers checkpoint at most 50 rows per invocation',
);
