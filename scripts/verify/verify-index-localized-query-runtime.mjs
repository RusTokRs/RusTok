#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-localized-query-runtime] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-index/src/application/query_port.rs', [
  'LocalizedBuild(#[from] PostgresLocalizedQueryBuildError)',
  'LocalizedDecode(#[from] PostgresLocalizedQueryDecodeError)',
  'async fn execute_localized_query(',
  'query: LocalizedEntityQuery',
  'localized Index query execution is unavailable for this adapter',
]);
requireMarkers('crates/rustok-index/src/application/query_runtime.rs', [
  'use crate::domain::{IndexQuery, LocalizedEntityQuery};',
  'async fn execute_localized_query(',
  'self.port.execute_localized_query(query).await',
]);

const postgres = requireMarkers('crates/rustok-index/src/infrastructure/postgres/query_port.rs', [
  'CompiledPostgresLocalizedPageQuery',
  'domain::{IndexQuery, LocalizedEntityQuery, SchemaRef}',
  'fn admitted_localized_page_query(',
  'self.apply_admissions(&query.query, page_query.compiled_mut())?;',
  'fn apply_admissions(',
  '.apply_link_target_availability(query, compiled)',
  'descriptor.admission().apply(compiled)',
  'async fn configure_snapshot_and_verify(',
  'READ_ONLY_SNAPSHOT_SQL',
  'verify_persisted_schemas(transaction, query, required_schemas).await',
  'async fn execute_localized_in_transaction(',
  'Self::configure_snapshot_and_verify(transaction, &query.query, required_schemas).await?;',
  'let page_rows = execute_page_rows(transaction, compiled).await?;',
  'execute_exact_count(transaction, count).await?',
  '.decode_postgres_localized_query_page(query, page_query, page_rows, exact_count_row)',
  'async fn execute_localized_query(',
  'let required_schemas = required_schema_contracts(&self.registry, &query.query)?;',
  '.compile_postgres_localized_page_query(&query)?;',
  'let page_query = self.admitted_localized_page_query(&query, page_query)?;',
  'begin localized query snapshot',
  'Self::finish_transaction(transaction, result).await',
]);
const admissionPosition = postgres.indexOf('let page_query = self.admitted_localized_page_query(&query, page_query)?;');
const beginPosition = postgres.indexOf('begin localized query snapshot');
if (admissionPosition < 0 || beginPosition < 0 || admissionPosition > beginPosition) {
  fail('localized owner admission must be prepared before the storage transaction begins');
}

requireMarkers('crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md', [
  'Status: `runtime_text_pattern_identity_order_source_complete_adapter_and_evidence_pending`',
  '`execute_localized_query`',
  '`REPEATABLE READ, READ ONLY`',
  '`PostgresQueryEntityAdmission`',
  '`identity_order_direction`',
]);

console.log('[verify-index-localized-query-runtime] localized fold runtime remains source-locked behind readiness/admission/snapshot semantics with explicit identity ordering');