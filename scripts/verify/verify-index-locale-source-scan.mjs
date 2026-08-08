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

requireMarkers('crates/rustok-index/docs/m6-locale-scoped-source-scan.md', [
  'Status: `generic_source_contract_complete_product_source_pending`.',
  '`IndexSourceScanRequest::new(...)`',
  '`IndexSourceScanRequest::for_locale(...)`',
  '`IndexSourceError::ScanMutationLocaleMismatch`',
  'constrain its underlying scan before pagination',
  'does not:',
  'add Product SQL locale filtering yet',
  'add `partition_key` behavior',
  'current Product',
]);

console.log('[verify-index-locale-source-scan] generic exact-locale source scan contract is fail-closed while durable replay scope and Product SQL remain separate');
