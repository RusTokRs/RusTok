#!/usr/bin/env node

import fs from 'node:fs';

const migrationPath =
  'crates/rustok-product/src/migrations/m20260829_000017_add_product_category_seo_translations.rs';
const migrationsModPath = 'crates/rustok-product/src/migrations/mod.rs';
const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const planPath = 'crates/rustok-taxonomy/docs/implementation-plan.md';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [
  migrationPath,
  migrationsModPath,
  categoriesPath,
  contractPath,
  localeContractPath,
  planPath,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const migration = fs.readFileSync(migrationPath, 'utf8');
  const migrationsMod = fs.readFileSync(migrationsModPath, 'utf8');
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(fs.readFileSync(localeContractPath, 'utf8'));
  const plan = normalizeWhitespace(fs.readFileSync(planPath, 'utf8'));

  for (const [marker, label] of [
    ['manager.get_database_backend() != DatabaseBackend::Postgres', 'PostgreSQL-only migration boundary'],
    ['CREATE TABLE IF NOT EXISTS catalog_category_seo_translations', 'dedicated Product Category SEO table'],
    ['tenant_id UUID NOT NULL', 'tenant-owned SEO identity'],
    ['category_id UUID NOT NULL', 'Product Category SEO identity'],
    ['locale VARCHAR(32) NOT NULL', 'localized SEO identity'],
    ['meta_title VARCHAR(255)', 'Product meta title storage'],
    ['meta_description VARCHAR(500)', 'Product meta description storage'],
    ['PRIMARY KEY (tenant_id, category_id, locale)', 'tenant/category/locale uniqueness'],
    ['FOREIGN KEY (tenant_id, category_id)', 'tenant-safe Product Category foreign key'],
    ['REFERENCES catalog_categories(tenant_id, id)', 'Product Category owner reference'],
    ['CHECK (meta_title IS NOT NULL OR meta_description IS NOT NULL)', 'no empty SEO-only rows'],
    ['Product Category SEO backfill blocked by incompatible existing SEO ownership', 'fail-closed incompatible SEO preflight'],
    ['seo.meta_title IS DISTINCT FROM translation.meta_title', 'meta title compatibility check'],
    ['seo.meta_description IS DISTINCT FROM translation.meta_description', 'meta description compatibility check'],
    ['FROM catalog_category_translations translation', 'legacy SEO backfill source'],
    ['JOIN catalog_categories category', 'tenant identity backfill source'],
    ['WHERE translation.meta_title IS NOT NULL', 'SEO-only backfill filter'],
    ['OR translation.meta_description IS NOT NULL', 'SEO description backfill filter'],
    ['ON CONFLICT (tenant_id, category_id, locale) DO NOTHING', 'monotonic compatible backfill'],
    ['drop_table(', 'reversible additive seam'],
  ]) {
    need(migration, marker, label);
  }
  forbid(migration, 'name VARCHAR', 'canonical Category name duplication in SEO storage');
  forbid(migration, 'description TEXT', 'canonical Category description duplication in SEO storage');

  for (const [marker, label] of [
    ['mod m20260829_000017_add_product_category_seo_translations;', 'migration module registration'],
    ['Box::new(m20260829_000017_add_product_category_seo_translations::Migration)', 'migration execution registration'],
  ]) {
    need(migrationsMod, marker, label);
  }

  const createStart = categories.indexOf('pub async fn create_category(');
  const listStart = categories.indexOf('pub async fn list_categories(');
  if (createStart < 0 || listStart <= createStart) {
    failures.push('Product create/list Category function boundaries are required');
  } else {
    const createBody = categories.slice(createStart, listStart);
    const donorTranslation = createBody.indexOf('INSERT INTO catalog_category_translations');
    const seoWrite = createBody.indexOf('write_category_seo_translation_in_tx(');
    const taxonomySync = createBody.indexOf('sync_created_category_to_taxonomy_in_tx(');
    const domainEvent = createBody.indexOf('DomainEvent::CatalogCategoryCreated');
    const commit = createBody.indexOf('txn.commit().await?');
    for (const [index, label] of [
      [donorTranslation, 'retained compatibility donor translation write'],
      [seoWrite, 'Product Category SEO write'],
      [taxonomySync, 'Taxonomy owner sync'],
      [domainEvent, 'Product Category event'],
      [commit, 'shared Product transaction commit'],
    ]) {
      if (index < 0) failures.push(`missing create ordering marker: ${label}`);
    }
    if (
      donorTranslation >= 0 &&
      seoWrite >= 0 &&
      taxonomySync >= 0 &&
      domainEvent >= 0 &&
      commit >= 0 &&
      !(donorTranslation < seoWrite && seoWrite < taxonomySync && taxonomySync < domainEvent && domainEvent < commit)
    ) {
      failures.push(
        'Product create must write compatibility copy, Product SEO, Taxonomy owner state, event, then commit',
      );
    }
  }

  const seoStart = categories.indexOf('async fn write_category_seo_translation_in_tx(');
  const syncStart = categories.indexOf('async fn sync_created_category_to_taxonomy_in_tx(');
  if (seoStart < 0 || syncStart <= seoStart) {
    failures.push('Product Category SEO/Taxonomy helper boundaries are required');
  } else {
    const seoHelper = categories.slice(seoStart, syncStart);
    for (const [marker, label] of [
      ['txn.get_database_backend() != DatabaseBackend::Postgres', 'runtime PostgreSQL-only SEO boundary'],
      ['!category_translation_has_seo(translation)', 'skip empty SEO translation'],
      ['INSERT INTO catalog_category_seo_translations', 'dedicated Product SEO write'],
      ['tenant_id, category_id, locale, meta_title, meta_description', 'complete Product SEO row'],
      ['translation.locale.clone()', 'normalized locale reuse'],
      ['translation.meta_title.clone()', 'meta title write'],
      ['translation.meta_description.clone()', 'meta description write'],
    ]) {
      need(seoHelper, marker, label);
    }
  }

  for (const [marker, label] of [
    ['fn category_translation_has_seo(', 'Product SEO presence helper'],
    ['translation.meta_title.is_some() || translation.meta_description.is_some()', 'Product SEO presence rule'],
    ['category_seo_detects_localized_metadata', 'Product SEO unit regression'],
    ['sync_created_category_to_taxonomy_in_tx(', 'retained Taxonomy create sync'],
    ['TaxonomyOwnerCategoryReader', 'retained CAT-26 Taxonomy read projection'],
  ]) {
    need(categories, marker, label);
  }
  forbid(categories, 'product/category', 'duplicate Product Category Translation provider');

  for (const [marker, label] of [
    ['Status: **source-complete Product Category SEO seam; canonical donor storage retirement pending**', 'CAT-27 bounded status'],
    ['`catalog_category_seo_translations` as dedicated Product-owned SEO storage', 'Product SEO ownership seam'],
    ['fails closed with an incompatible ownership error', 'SEO migration conflict policy'],
    ['does **not** drop `catalog_category_translations`', 'no premature donor retirement'],
    ['Any SEO insert or later Taxonomy failure rolls the whole Product create back', 'transactional SEO create boundary'],
    ['No `product/category` Translation provider is introduced', 'no duplicate Translation provider'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-27 isolates Product-only localized SEO', 'locale CAT-27 status'],
    ['`catalog_category_seo_translations` is the Product-owned localized SEO store', 'locale SEO owner'],
    ['fails closed if an already-present SEO row', 'locale backfill conflict policy'],
    ['before the existing Taxonomy owner-sync', 'SEO create ordering'],
    ['does **not** drop `catalog_category_translations`', 'locale no premature retirement'],
  ]) {
    need(localeContract, marker, label);
  }

  for (const [marker, label] of [
    ['### TAXONOMY-CAT-6 — Product and later consumers — IN PROGRESS', 'actualized Product migration cursor'],
    ['PR #3735 / TAXONOMY-CAT-23', 'Product binding slice history'],
    ['PR #3736 / TAXONOMY-CAT-24', 'Product backfill slice history'],
    ['PR #3737 / TAXONOMY-CAT-25', 'Product create slice history'],
    ['PR #3738 / TAXONOMY-CAT-26', 'Product read slice history'],
    ['TAXONOMY-CAT-27 isolates those SEO fields', 'Product SEO next slice'],
    ['separate fail-closed donor-retirement slice', 'Product donor retirement sequencing'],
  ]) {
    need(plan, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-category-seo-seam] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-category-seo-seam] Product Category localized SEO ownership seam verified');
