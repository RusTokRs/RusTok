#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-attribute-term-contract] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const modulePath = 'crates/rustok-distribution/src/product_index/attribute_terms.rs';
const source = requireMarkers(modulePath, [
  'pub(crate) const PRODUCT_ATTRIBUTE_TERMS_FIELD: &str = "attribute_terms";',
  'pub(crate) const PRODUCT_ATTRIBUTE_TERMS_CTE: &str',
  "pa.archived_at IS NULL",
  'pa.is_filterable = TRUE',
  "pa.scope IN ('product', 'both')",
  'pav.detached_at IS NULL',
  "value.value_type IN ('text', 'textarea', 'richtext')",
  "value.attribute_id::text || '|text||'",
  "value.attribute_id::text || '|localized_text|'",
  "value.attribute_id::text || '|localized_present|'",
  "value.attribute_id::text || '|integer||'",
  "value.attribute_id::text || '|decimal||'",
  'trim_scale(value.value_decimal)::text',
  "value.attribute_id::text || '|boolean||'",
  "value.attribute_id::text || '|date||'",
  "value.attribute_id::text || '|datetime||'",
  'extract(epoch FROM value.value_datetime) * 1000000',
  "value.attribute_id::text || '|option||'",
  'product_attribute_value_options option_value',
  'SELECT DISTINCT product_id, term',
  'jsonb_agg(term ORDER BY term)',
  'rustok_product::product_attribute_text_term(',
  'rustok_product::product_attribute_localized_text_term(',
  'rustok_product::product_attribute_decimal_term(',
  'rustok_product::product_attribute_datetime_term(',
  'pub(crate) fn localized_text_filter(',
  'FilterExpr::Or(vec![',
  'FilterExpr::Not(Box::new(requested_present))',
]);
if (source.includes('pa.code ||') || source.includes('pa.code::text ||')) {
  fail(`${modulePath} must key persisted terms by stable attribute UUID rather than mutable public code`);
}

requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'hex_encode(value)',
  'value.normalize().to_string()',
  'value.timestamp_micros().to_string()',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'mod attribute_terms;',
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
]);
requireMarkers('crates/rustok-distribution/Cargo.toml', [
  'mod-product = ["dep:rustok-product", "mod-taxonomy", "dep:chrono", "dep:hex", "dep:rust_decimal"]',
  'chrono = { workspace = true, optional = true }',
  'hex = { workspace = true, optional = true }',
  'rust_decimal = { workspace = true, optional = true }',
]);

requireMarkers('crates/rustok-product/src/services/catalog/attribute_filters.rs', [
  'AND archived_at IS NULL',
  'AND is_filterable = TRUE',
  "AND scope IN ('product', 'both')",
  'AND pav.detached_at IS NULL',
  'parse_product_attribute_filter_value(',
  'NOT EXISTS (',
  'requested_any',
  'pao.archived_at IS NULL',
]);

requireMarkers('crates/rustok-product/src/services/catalog_attribute_terms.rs', [
  'AttributeValueType::Text | AttributeValueType::Textarea | AttributeValueType::Richtext',
  'AttributeValueType::Integer',
  'AttributeValueType::Decimal',
  'AttributeValueType::Boolean',
  'AttributeValueType::Date',
  'AttributeValueType::Datetime',
  'AttributeValueType::Select | AttributeValueType::Multiselect',
  'AttributeValueType::Json',
]);

requireMarkers('crates/rustok-product/src/services/write_transaction.rs', [
  'DomainEvent::ProductAttributeValuesChanged { product_id } => Some(*product_id)',
  'UPDATE products SET index_revision = index_revision',
]);

requireMarkers('crates/rustok-distribution/src/product_index/product.rs', [
  'many_field("attribute_terms", IndexValueType::String, false, true)?',
  'PRODUCT_ATTRIBUTE_TERMS_CTE',
  "COALESCE(attributes.attribute_terms, '[]'::jsonb) AS attribute_terms",
  'decode_string_json_list(&row, "attribute_terms")?',
  'field_name("attribute_terms")?',
  'derive_index_schema_source_event_id(',
]);

requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`attribute_terms: Many<String>`',
  '`<attribute_uuid>|<kind>|<locale_hex>|<value_hex>`',
  'There is deliberately no format-version prefix.',
  '`localized_present`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
  '`derive_index_schema_source_event_id`',
  'numeric key is an internal storage/replay identity',
]);

console.log('[verify-index-product-attribute-term-contract] canonical Product typed EAV terms are materialized by the single current source; rebuild remains pending');
