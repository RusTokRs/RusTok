#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-product-storefront-attribute-filter-terms] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const terms = requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'pub enum ProductAttributeTermExpr',
  'Term(String)',
  'And(Vec<ProductAttributeTermExpr>)',
  'Or(Vec<ProductAttributeTermExpr>)',
  'Not(Box<ProductAttributeTermExpr>)',
  'Never',
  'pub struct ProductResolvedAttributeFilter',
  'pub(crate) fn parse_product_attribute_filter_value(',
  '"true" | "1"',
  '"false" | "0"',
  'NaiveDate::parse_from_str(raw_value, "%Y-%m-%d")',
  'DateTime::parse_from_rfc3339(raw_value)',
  'value.normalize().to_string()',
  'value.timestamp_micros().to_string()',
  'product_attribute_localized_text_expr(',
  'ProductAttributeTermExpr::Not(Box::new(requested_present))',
  'normalize_locale_tag(trimmed)',
]);
if (terms.includes('rustok_index')) {
  fail('Product-owned canonical term contract must not depend on rustok-index');
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub struct ProductAttributeFilter',
  'pub(crate) fn validate_product_attribute_filters(',
  'MAX_ATTRIBUTE_FILTERS',
  'MAX_ATTRIBUTE_FILTER_CODE_LENGTH',
  'MAX_ATTRIBUTE_FILTER_VALUE_LENGTH',
  'filter.code.to_ascii_lowercase()',
]);

requireMarkers('crates/rustok-product/src/services/catalog_schema_service/attributes.rs', [
  'pub async fn resolve_storefront_attribute_filter_terms(',
  'validate_product_attribute_filters(filters)?;',
  'archived_at IS NULL',
  'is_filterable = TRUE',
  "scope IN ('product', 'both')",
  'LOWER(code) IN',
  'parse_product_attribute_filter_value(',
  'product_attribute_localized_text_expr(',
  'Uuid::parse_str(raw_value.as_str())',
  'Ok(option_id) if option_id.is_nil() => return Ok(ProductAttributeTermExpr::Never)',
  'AND archived_at IS NULL',
  'AND code = {code_placeholder}',
  'return Ok(ProductAttributeTermExpr::Never);',
]);

requireMarkers('crates/rustok-product/src/services/catalog/attribute_filters.rs', [
  'validate_product_attribute_filters(filters)?;',
  'parse_product_attribute_filter_value(',
  'ProductAttributeFilterValue::Boolean(value)',
  'ProductAttributeFilterValue::Option(value)',
]);

requireMarkers('crates/rustok-product/src/catalog_schema_read_port.rs', [
  'pub struct ProductStorefrontAttributeFilterResolutionRequest',
  'async fn resolve_storefront_attribute_filters(',
  '"product.storefront_attribute_filter_resolution_unavailable"',
  'ProductCatalogSchemaService::resolve_storefront_attribute_filter_terms(',
  'context.locale.as_str()',
  'request.fallback_locale.as_str()',
]);

const distribution = requireMarkers('crates/rustok-distribution/src/product_index/attribute_terms.rs', [
  'pub(crate) use rustok_product::ProductAttributeTermError;',
  'rustok_product::product_attribute_text_term(',
  'rustok_product::product_attribute_localized_text_term(',
  'rustok_product::product_attribute_option_term(',
  'PRODUCT_ATTRIBUTE_TERMS_CTE',
  "value.attribute_id::text || '|localized_text|'",
  "value.attribute_id::text || '|option||'",
]);
if (distribution.includes('fn product_attribute_term(')) {
  fail('distribution must not own a second Rust canonical Product term encoder');
}

console.log('[verify-product-storefront-attribute-filter-terms] Product owns Storefront filter parsing/term identity; port remains optional and distribution delegates Rust term grammar');
