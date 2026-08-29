#!/usr/bin/env node

import fs from 'node:fs';

const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const ownerReadPath = 'crates/rustok-taxonomy/src/owner_category_read.rs';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [categoriesPath, contractPath, localeContractPath, ownerReadPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(fs.readFileSync(localeContractPath, 'utf8'));
  const ownerRead = fs.readFileSync(ownerReadPath, 'utf8');

  const listStart = categories.indexOf('pub async fn list_categories(');
  const groupStart = categories.indexOf('pub async fn create_category_group(');
  if (listStart < 0 || groupStart <= listStart) {
    failures.push('Product list/create-group Category function boundaries are required');
  } else {
    const listBody = categories.slice(listStart, groupStart);
    need(
      listBody,
      'self.db.get_database_backend() == DatabaseBackend::Postgres',
      'PostgreSQL-only Taxonomy list cutover',
    );
    need(
      listBody,
      'list_categories_from_taxonomy(self, tenant_id, &locale).await',
      'PostgreSQL Taxonomy projection dispatch',
    );
    need(
      listBody,
      'list_categories_from_product_donor(self, tenant_id, &locale).await',
      'non-PostgreSQL donor fallback',
    );
  }

  const taxonomyReadStart = categories.indexOf('async fn list_categories_from_taxonomy(');
  const composeStart = categories.indexOf('fn compose_taxonomy_category_list_records(');
  const donorReadStart = categories.indexOf('async fn list_categories_from_product_donor(');
  if (
    taxonomyReadStart < 0 ||
    composeStart <= taxonomyReadStart ||
    donorReadStart <= composeStart
  ) {
    failures.push('Product Taxonomy/donor list helper boundaries are required');
  } else {
    const taxonomyRead = categories.slice(taxonomyReadStart, composeStart);
    const compose = categories.slice(composeStart, donorReadStart);
    const donorRead = categories.slice(donorReadStart);

    for (const [marker, label] of [
      ['ProductCategoryTaxonomyReadRow::find_by_statement', 'bounded Product composition read'],
      ['LEFT JOIN product_catalog_category_taxonomy_bindings binding', 'typed Product binding read'],
      ['binding.taxonomy_category_id', 'bound Taxonomy identity projection'],
      ['ORDER BY c.path ASC', 'retained Product path ordering'],
      ['missing its Taxonomy Category binding', 'missing binding fail-closed'],
      ['bound to incompatible Taxonomy Category', 'same-ID binding fail-closed'],
      ['TaxonomyOwnerCategoryReader::new(service.db.clone())', 'Taxonomy owner reader'],
      ['TaxonomyScopeType::Module', 'Taxonomy module scope'],
      ['Some(PRODUCT_TAXONOMY_SCOPE)', 'Product Taxonomy scope value'],
      ['Some(&taxonomy_category_ids)', 'bounded owner ID read'],
      ['Some(PLATFORM_FALLBACK_LOCALE)', 'platform locale fallback'],
    ]) {
      need(taxonomyRead, marker, label);
    }
    forbid(
      taxonomyRead,
      'catalog_category_translations',
      'PostgreSQL donor localized-copy read',
    );
    forbid(taxonomyRead, 'c.parent_id', 'Product parent as canonical PostgreSQL read');
    forbid(taxonomyRead, 'c.slug', 'Product slug as canonical PostgreSQL read');

    for (const [marker, label] of [
      ['owner.parent_id', 'Taxonomy parent composition'],
      ['slug: owner.slug', 'Taxonomy canonical slug composition'],
      ['name: owner.name', 'Taxonomy canonical name composition'],
      ['code: row.code', 'retained Product code'],
      ['path: row.path', 'retained Product path'],
      ['kind: row.kind', 'retained Product kind'],
      ['canonical_key_for_product_category(row.id)', 'same-ID canonical key verification'],
      ['owner.scope_type != TaxonomyScopeType::Module', 'Taxonomy scope fail-closed'],
      ['owner.available_locales.is_empty()', 'missing canonical localized copy fail-closed'],
      ['missing its Taxonomy owner projection', 'missing owner row fail-closed'],
    ]) {
      need(compose, marker, label);
    }

    need(
      donorRead,
      'FROM catalog_category_translations translation',
      'retained non-PostgreSQL donor localized fallback',
    );
  }

  for (const [marker, label] of [
    ['category_taxonomy_read_composes_owner_copy', 'owner composition unit regression'],
    ['category_taxonomy_read_rejects_missing_owner', 'fail-closed unit regression'],
    ['Product Category Taxonomy read projection failed:', 'Taxonomy read error mapping'],
    ['sync_created_category_to_taxonomy_in_tx(', 'retained CAT-25 create dual-write'],
  ]) {
    need(categories, marker, label);
  }
  forbid(categories, 'product/category', 'duplicate Product Category Translation provider');

  for (const [marker, label] of [
    ['pub struct TaxonomyOwnerCategoryReader', 'Taxonomy owner reader surface'],
    ['pub async fn load_scoped_categories(', 'bounded Taxonomy owner read API'],
    ['TaxonomyTermKind::Category', 'Category kind filter'],
    ['taxonomy_term::Column::TenantId.eq(tenant_id)', 'tenant filter'],
    ['resolve_by_locale_with_fallback(', 'shared deterministic locale policy'],
    ['taxonomy_category_hierarchy::Entity::find()', 'Taxonomy hierarchy projection'],
  ]) {
    need(ownerRead, marker, label);
  }

  for (const [marker, label] of [
    ['Status: **source-complete PostgreSQL Taxonomy read projection; donor storage retirement pending**', 'CAT-26 bounded status'],
    ['requires every live Product Category to have a same-ID binding', 'binding fail-closed contract'],
    ['localized `name`, localized canonical `slug` and `parent_id` from the Taxonomy owner projection', 'Taxonomy canonical read ownership'],
    ['retains Product `code`, `kind` and `path`', 'retained Product composition ownership'],
    ['PostgreSQL list path no longer reads `catalog_category_translations`', 'donor localized read retirement'],
    ['Other backends continue using the retained Product donor list path', 'non-PostgreSQL compatibility boundary'],
    ['No `product/category` Translation provider is introduced', 'no duplicate Translation provider'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-26 switches the PostgreSQL Category list projection', 'locale read cutover status'],
    ['`TaxonomyOwnerCategoryReader` supplies canonical localized `name`', 'Taxonomy localized owner'],
    ['requested locale, platform fallback locale, then the lexicographically smallest normalized available locale', 'fallback parity'],
    ['fails closed rather than using `catalog_category_translations` as a hidden fallback', 'no donor fallback on PostgreSQL'],
    ['On non-PostgreSQL backends the existing donor read remains active', 'cross-backend boundary'],
    ['Product `path` continues to define list ordering', 'retained Product ordering'],
  ]) {
    need(localeContract, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-read-cutover] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-taxonomy-category-read-cutover] PostgreSQL Taxonomy Category read projection verified');
