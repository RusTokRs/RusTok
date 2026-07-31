#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  mapper: 'crates/rustok-product/src/public_error.rs',
  error: 'crates/rustok-product/src/error.rs',
  root: 'crates/rustok-product/src/lib.rs',
  evidence:
    'crates/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source.json',
  review:
    'crates/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source-review.json',
  document: 'crates/rustok-product/docs/product-public-error-diagnostic-safety.md',
};

const mapperSource = read(paths.mapper);
const errorSource = read(paths.error);
const rootSource = read(paths.root);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const document = read(paths.document);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

for (const marker of [
  "pub struct ProductPublicError {",
  "pub message: &'static str",
  "pub code: &'static str",
  'pub retryable: bool',
  'pub correlation_id: Uuid',
  'impl std::fmt::Display for ProductPublicError',
  '"{} (code: {}; reference: {})"',
  'pub fn map_product_public_error(',
]) requireText(mapperSource, marker, `${paths.mapper}: public contract`);
requireText(
  rootSource,
  'pub use public_error::{ProductPublicError, map_product_public_error};',
  `${paths.root}: shared export`,
);

for (const marker of [
  'Database(#[from] sea_orm::DbErr)',
  'ProductNotFound(Uuid)',
  'DuplicateHandle { handle: String, locale: String }',
  'DuplicateSku(String)',
  'Validation(String)',
  'NoVariants',
  'CannotDeletePublished',
  'Core(#[from] CoreError)',
]) requireText(errorSource, marker, `${paths.error}: owner error shape`);

const facts = functionBody(mapperSource, 'product_owner_error_facts');
for (const marker of [
  'struct ProductOwnerErrorFacts',
  'CommerceError::Database(_) => ("database", 0, 0, 0, 0, true)',
  'CommerceError::ProductNotFound(id) =>',
  '"product_not_found"',
  'if id.is_nil() { 0 } else { 1 }',
  'CommerceError::DuplicateHandle { handle, locale } =>',
  'handle.chars().count() + locale.chars().count()',
  'CommerceError::DuplicateSku(sku) =>',
  'sku.chars().count()',
  'CommerceError::Validation(message) =>',
  'message.chars().count()',
  'CommerceError::NoVariants => ("no_variants", 0, 0, 0, 0, false)',
  'CommerceError::CannotDeletePublished =>',
  '"cannot_delete_published"',
  'CommerceError::Core(_) => ("core", 0, 0, 0, 0, true)',
]) requireText(mapperSource + facts, marker, `${paths.mapper}: bounded owner facts`);

const mapper = functionBody(mapperSource, 'map_product_public_error');
for (const marker of [
  'CommerceError::Database(_) =>',
  '"Product data is temporarily unavailable"',
  '"PRODUCT_TEMPORARILY_UNAVAILABLE"',
  'CommerceError::ProductNotFound(_) =>',
  '"Product was not found"',
  '"PRODUCT_NOT_FOUND"',
  'CommerceError::DuplicateHandle { .. } =>',
  '"Product handle conflicts with an existing product"',
  '"DUPLICATE_HANDLE"',
  'CommerceError::DuplicateSku(_) =>',
  '"Product SKU conflicts with an existing product"',
  '"DUPLICATE_SKU"',
  'CommerceError::Validation(_) =>',
  '"Product request is invalid"',
  '"PRODUCT_VALIDATION"',
  'CommerceError::NoVariants =>',
  '"Product requires at least one variant"',
  '"NO_VARIANTS"',
  'CommerceError::CannotDeletePublished =>',
  '"Published products must be archived before removal"',
  '"CANNOT_DELETE_PUBLISHED"',
  'CommerceError::Core(_) =>',
  '"Product operation could not be completed safely"',
  '"PRODUCT_OPERATION_FAILED"',
  'let correlation_id = Uuid::new_v4();',
  'let error_facts = product_owner_error_facts(error);',
  'tracing::error!(',
  'error_variant = error_facts.error_variant',
  'text_field_count = error_facts.text_field_count',
  'text_total_length = error_facts.text_total_length',
  'uuid_field_count = error_facts.uuid_field_count',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'operation,',
  'public_code = code',
  'retryable,',
  'boundary,',
  '%correlation_id',
  'ProductPublicError {',
  'message,',
  'code,',
  'correlation_id,',
]) requireText(mapper, marker, `${paths.mapper}: stable shared mapper`);

for (const forbidden of [
  'error = ?error',
  'error = %error',
  'error = ?message',
  'error = %message',
  'handle = %',
  'handle = ?',
  'locale = %',
  'locale = ?',
  'sku = %',
  'sku = ?',
  'product_id = %',
  'product_id = ?',
]) forbidText(mapper, forbidden, `${paths.mapper}: owner payload diagnostics`);

for (const [key, expected] of Object.entries({
  complete_commerce_error_logged: false,
  database_error_payload_logged: false,
  core_error_payload_logged: false,
  handle_locale_text_logged: false,
  sku_validation_text_logged: false,
  product_uuid_logged: false,
  static_error_variant_logged: true,
  aggregate_text_shape_logged: true,
  aggregate_uuid_shape_logged: true,
  opaque_payload_presence_logged: true,
  public_message_changed: false,
  public_code_changed: false,
  public_retryability_changed: false,
  correlation_generation_changed: false,
  correlation_rendering_changed: false,
  shared_export_changed: false,
  shared_consumer_contracts_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'verifiers_run',
  'cargo_run',
  'format_run',
  'workflow_checks_run',
  'ci_run',
  'compile_proven',
  'mounted_runtime_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  public_struct_preserved: true,
  display_contract_preserved: true,
  all_public_mappings_preserved: true,
  correlation_envelope_preserved: true,
  complete_commerce_error_logging_removed: true,
  database_core_payload_removed: true,
  product_uuid_removed: true,
  handle_locale_sku_validation_text_removed: true,
  bounded_error_shape_retained: true,
  shared_graphql_native_consumers_unchanged: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  '`map_product_public_error`',
  'GraphQL and native admin/storefront',
  'all eight current `CommerceError` variants',
  'broader ecommerce mapper cleanup remains open',
]) requireText(document, marker, `${paths.document}: truthful scope`);

if (failures.length > 0) {
  console.error('Product shared public-error diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Product shared public errors preserve public mappings and correlation while retaining only bounded owner-error facts; execution evidence remains open',
);
