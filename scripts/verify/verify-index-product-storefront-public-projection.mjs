#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-public-projection] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
requireMarkers(ownerPath, [
  'title: translation',
  '.unwrap_or_else(|| "Untitled product".to_string())',
  'handle: translation',
  '.unwrap_or_default()',
]);

const rawPacketPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_postgres_tests.rs';
requireMarkers(rawPacketPath, [
  'assert_eq!(owner_c.title, "Untitled product");',
  'assert_eq!(owner_c.handle, "");',
  'assert_eq!(projected_string(index_c, "title")?, None);',
  'assert_eq!(projected_string(index_c, "handle")?, None);',
]);

const projectionPath = 'crates/rustok-distribution/src/product_index/storefront_projection.rs';
const projection = requireMarkers(projectionPath, [
  'const UNTITLED_PRODUCT: &str = "Untitled product";',
  'pub(crate) enum ProductStorefrontIndexPublicProjectionError',
  'MissingField',
  'DuplicateField',
  'InvalidFieldValue',
  'pub(crate) fn project_product_storefront_index_page(',
  'apply_string_placeholder(item, "title", UNTITLED_PRODUCT)?;',
  'apply_string_placeholder(item, "handle", "")?;',
  'projected.exact_count, Some(9)',
  'projected.next_cursor.as_deref(), Some("opaque-cursor")',
  'value(&projected.items[0], "tag_ids")',
  'IndexValue::List(vec![IndexValue::Uuid(tag_id)])',
  'fails_closed_on_missing_duplicate_or_wrong_typed_public_fields',
]);
for (const forbidden of [
  'FilterExpr',
  'OrderExpr',
  'Pagination::',
  'LocalizedEntityQuery',
  'execute_localized_query',
]) {
  if (projection.includes(forbidden)) {
    fail(`${projectionPath} must remain a post-page transform; found query/execution marker ${forbidden}`);
  }
}

const builderPath = 'crates/rustok-distribution/src/product_index/storefront_shadow.rs';
const builder = read(builderPath);
for (const forbidden of ['Untitled product', 'project_product_storefront_index_page']) {
  if (builder.includes(forbidden)) {
    fail(`${builderPath} must not feed owner public placeholders into query construction: ${forbidden}`);
  }
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>',
  'pub(crate) public_projected:',
  'Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>',
  'let projected = self',
  '.execute_projected(',
  'let public_projected = projected',
  '.as_ref()',
  '.ok()',
  '.cloned()',
  '.map(project_product_storefront_index_page);',
  'let comparison = projected',
  'compare_owner_and_index(&authoritative, projected)',
]);
const executeProjectedStart = executor.indexOf('async fn execute_projected(');
const compareStart = executor.indexOf('fn compare_owner_and_index(');
if (executeProjectedStart < 0 || compareStart <= executeProjectedStart) {
  fail(`${executorPath} projected execution/comparison boundaries are missing`);
}
const rawExecution = executor.slice(executeProjectedStart, compareStart);
if (rawExecution.includes('project_product_storefront_index_page')) {
  fail(`${executorPath} owner public placeholders must be applied after raw Index execution, not inside it`);
}
const rawPosition = executor.indexOf('let projected = self');
const publicPosition = executor.indexOf('let public_projected = projected');
const comparisonPosition = executor.indexOf('let comparison = projected');
if (
  rawPosition < 0 ||
  publicPosition <= rawPosition ||
  comparisonPosition <= rawPosition
) {
  fail('raw Index page must exist before public projection and raw equivalence comparison');
}

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod storefront_projection;',
  'ProductStorefrontIndexPublicProjectionError',
  'project_product_storefront_index_page',
]);

console.log('[verify-index-product-storefront-public-projection] Product public placeholders are post-page only; raw Index null evidence and tag_ids remain unchanged');
