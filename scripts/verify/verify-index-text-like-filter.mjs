#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-text-like-filter] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const queryPath = 'crates/rustok-index/src/domain/query.rs';
const query = requireMarkers(queryPath, [
  'IsNull(FieldPath, bool)',
  'TextLike(FieldPath, String)',
  'Self::TextLike(path, _) => output.push(path)',
]);
if (query.indexOf('TextLike(FieldPath, String)') < query.indexOf('IsNull(FieldPath, bool)')) {
  fail(`${queryPath} must append TextLike after existing filter variants to preserve postcard discriminants`);
}

requireMarkers('crates/rustok-index/src/application/validation.rs', [
  'const MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024;',
  'TextLikePatternTooLong { maximum: usize, actual: usize }',
  'TextLikePatternContainsNul',
  'TextLikePatternDanglingEscape',
  'FilterExpr::TextLike(path, pattern)',
  'resolved.field.cardinality != FieldCardinality::One',
  'resolved.field.value_type != IndexValueType::String',
  'operator: "text_like"',
  'validate_text_like_pattern(pattern)?;',
  "pattern.contains('\\0')",
  "character == '\\\\'",
]);

const ordinary = requireMarkers('crates/rustok-index/src/application/postgres_query_sql.rs', [
  'FilterExpr::TextLike(path, pattern) => compile_text_like(plan, path, pattern, bindings)',
  'fn compile_text_like(',
  'compile_many_exists(plan, field, bindings, |sql, bindings|',
  'PostgresBindValue::Text(pattern.to_owned())',
  " LIKE {pattern} ESCAPE E'\\\\\\\\'",
]);
if (ordinary.includes('rustok-product')) {
  fail('ordinary TextLike compiler must remain Product-agnostic');
}

const localized = requireMarkers('crates/rustok-index/src/application/postgres_localized_query.rs', [
  'FilterExpr::TextLike(path, pattern)',
  'PostgresBindValue::Text(pattern.clone())',
  " LIKE {pattern} ESCAPE E'\\\\\\\\'",
  'FilterExpr::TextLike(',
  '"%needle%".to_owned()',
]);
if (localized.includes('product_title_search_condition')) {
  fail('localized TextLike compiler must not depend on Product owner code');
}

requireMarkers('crates/rustok-index/src/application/reference.rs', [
  'FilterExpr::TextLike(path, pattern)',
  'fn text_like_matches(value: &str, pattern: &str) -> bool',
  "'%' => tokens.push(TextLikeToken::AnyMany)",
  "'_' => tokens.push(TextLikeToken::AnyOne)",
  'text_like_matches_postgres_wildcards_and_escape',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests/reference_fixture.rs', [
  'FilterExpr::TextLike(path, pattern)',
  'fn text_like_matches(value: &str, pattern: &str) -> bool',
]);

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'let pattern = format!("%{search}%");',
  'FROM product_translations pt',
  'pt.product_id = products.id',
  'pt.title LIKE $1',
]);
const searchStart = owner.indexOf('fn product_title_search_condition(');
if (searchStart < 0) fail(`${ownerPath} title-search helper is missing`);
if (owner.slice(searchStart).includes('pt.locale')) {
  fail(`${ownerPath} title search became locale-scoped; revisit localized Storefront parity in the same PR`);
}

console.log('[verify-index-text-like-filter] bounded generic PostgreSQL LIKE semantics are source-locked for ordinary and folded filters');
