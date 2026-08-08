#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-locale-source-scan] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const sourcePath = 'crates/rustok-index/src/application/source_registry.rs';
const source = requireMarkers(sourcePath, [
  'locale: Option<LocaleKey>',
  'pub fn for_locale(',
  'Self::new_scoped(tenant_id, schema, Some(locale), cursor, limit)',
  'Self::new_scoped(tenant_id, schema, None, cursor, limit)',
  'pub fn locale(&self) -> Option<&LocaleKey>',
  'ScanMutationLocaleMismatch { position: usize }',
  'key.locale.as_ref() != Some(locale)',
  'return Err(IndexSourceError::ScanMutationLocaleMismatch { position });',
  'LocaleKey::new("EN-us")',
  '"en-US"',
]);

const schemaWide = source.indexOf('pub fn new(');
const localeScoped = source.indexOf('pub fn for_locale(', schemaWide);
const validator = source.indexOf('fn validate_scan_page(', localeScoped);
if (schemaWide < 0 || localeScoped <= schemaWide || validator <= localeScoped) {
  fail('schema-wide constructor, locale constructor and common page validator must remain ordered in the generic source contract');
}

for (const forbidden of [
  'partition_key',
  'scope_kind = \'locale\'',
  'IndexReplayRunRequest',
  'IndexReplayCheckpointKey',
  'IndexReplayJobLeaseRequest',
]) {
  if (source.includes(forbidden)) {
    fail(`${sourcePath} must remain a source-scan contract and not absorb durable replay scope: ${forbidden}`);
  }
}

const productPath = 'crates/rustok-distribution/src/product_index/product.rs';
const product = requireMarkers(productPath, [
  'locale_mode: LocaleMode::Required',
  'match (request.locale(), cursor)',
  '(Some(locale), Some(cursor)) =>',
  'WHERE row.locale = $2 AND row.product_id > $3',
  'ORDER BY row.product_id ASC\\nLIMIT $4'.replace('\\\\n', '\\n'),
  '(Some(locale), None) =>',
  'WHERE row.locale = $2',
  'ORDER BY row.product_id ASC\\nLIMIT $3'.replace('\\\\n', '\\n'),
  '(None, Some(cursor)) =>',
  'WHERE (row.product_id, row.locale) > ($2, $3)',
  'ORDER BY row.product_id ASC, row.locale ASC\\nLIMIT $4'.replace('\\\\n', '\\n'),
  '(None, None) =>',
  'ORDER BY row.product_id ASC, row.locale ASC\\nLIMIT $2'.replace('\\\\n', '\\n'),
  'locale.as_str().to_owned().into()',
  'if let (Some(locale), Some(cursor)) = (request.locale(), cursor.as_ref())',
  'cursor.locale != locale.as_str()',
  'return Err(permanent("product_index_cursor_invalid"));',
  'IndexSourcePage::new(&request, mutations, next_cursor)',
]);

const localeCursor = product.indexOf('(Some(locale), Some(cursor)) =>');
const localeFirst = product.indexOf('(Some(locale), None) =>', localeCursor);
const schemaCursor = product.indexOf('(None, Some(cursor)) =>', localeFirst);
const schemaFirst = product.indexOf('(None, None) =>', schemaCursor);
if (localeCursor < 0 || localeFirst <= localeCursor || schemaCursor <= localeFirst || schemaFirst <= schemaCursor) {
  fail('Product scan must retain four explicit locale/schema-wide cursor branches');
}

if (product.includes('filter(|row| row.locale')) {
  fail('Product locale replay must constrain SQL before pagination rather than post-filter rows');
}
if (product.includes('partition_key') || product.includes('scope_kind = \'locale\'')) {
  fail('Product source slice must not absorb durable locale job/checkpoint or partition scope');
}

requireMarkers('crates/rustok-index/docs/m6-locale-scoped-source-scan.md', [
  'Status: `product_source_complete_durable_replay_scope_pending`.',
  '`IndexSourceScanRequest::new(...)`',
  '`IndexSourceScanRequest::for_locale(...)`',
  '`IndexSourceError::ScanMutationLocaleMismatch`',
  'constrain its underlying scan before pagination',
  'Current Product',
  '`WHERE row.locale = $2`',
  '`WHERE row.locale = $2 AND row.product_id > $3`',
  '`product_index_cursor_invalid`',
  'write non-empty `index_jobs.locale_key` or `index_checkpoints.locale_key`',
  'add `partition_key` behavior',
  'durable locale replay identity',
]);

console.log('[verify-index-locale-source-scan] generic and Product exact-locale source scans are fail-closed before pagination while durable replay locale identity remains separate');
