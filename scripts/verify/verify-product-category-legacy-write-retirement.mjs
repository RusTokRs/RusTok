#!/usr/bin/env node

import fs from 'node:fs';

const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const planPath = 'crates/rustok-taxonomy/docs/implementation-plan.md';
const retainedSeoWorkflowPath = '.github/workflows/product-category-seo-seam.yml';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [
  categoriesPath,
  contractPath,
  localeContractPath,
  planPath,
  retainedSeoWorkflowPath,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(fs.readFileSync(localeContractPath, 'utf8'));
  const plan = normalizeWhitespace(fs.readFileSync(planPath, 'utf8'));
  const retainedSeoWorkflow = fs.readFileSync(retainedSeoWorkflowPath, 'utf8');

  const createStart = categories.indexOf('pub async fn create_category(');
  const listStart = categories.indexOf('pub async fn list_categories(');
  if (createStart < 0 || listStart <= createStart) {
    failures.push('Product create/list Category function boundaries are required');
  } else {
    const createBody = categories.slice(createStart, listStart);
    const compatibilityGuard = createBody.indexOf(
      'if should_write_legacy_category_translation(txn.get_database_backend())',
    );
    const donorTranslation = createBody.indexOf('INSERT INTO catalog_category_translations');
    const seoWrite = createBody.indexOf('write_category_seo_translation_in_tx(');
    const taxonomySync = createBody.indexOf('sync_created_category_to_taxonomy_in_tx(');
    const domainEvent = createBody.indexOf('DomainEvent::CatalogCategoryCreated');
    const commit = createBody.indexOf('txn.commit().await?');

    for (const [index, label] of [
      [compatibilityGuard, 'backend-bounded legacy write guard'],
      [donorTranslation, 'retained non-PostgreSQL donor write'],
      [seoWrite, 'Product SEO write'],
      [taxonomySync, 'Taxonomy owner sync'],
      [domainEvent, 'Product Category event'],
      [commit, 'shared Product transaction commit'],
    ]) {
      if (index < 0) failures.push(`missing create ordering marker: ${label}`);
    }
    if (
      compatibilityGuard >= 0 &&
      donorTranslation >= 0 &&
      seoWrite >= 0 &&
      taxonomySync >= 0 &&
      domainEvent >= 0 &&
      commit >= 0 &&
      !(
        compatibilityGuard < donorTranslation &&
        donorTranslation < seoWrite &&
        seoWrite < taxonomySync &&
        taxonomySync < domainEvent &&
        domainEvent < commit
      )
    ) {
      failures.push(
        'Product create must guard the legacy write, then write SEO, sync Taxonomy, publish, and commit',
      );
    }
  }

  const legacyHelperStart = categories.indexOf(
    'fn should_write_legacy_category_translation(',
  );
  const syncStart = categories.indexOf('async fn sync_created_category_to_taxonomy_in_tx(');
  if (legacyHelperStart < 0 || syncStart <= legacyHelperStart) {
    failures.push('legacy write/Taxonomy helper boundaries are required');
  } else {
    const legacyHelper = categories.slice(legacyHelperStart, syncStart);
    need(
      legacyHelper,
      'backend != DatabaseBackend::Postgres',
      'PostgreSQL legacy canonical write retirement',
    );
  }

  const listBody = categories.slice(listStart);
  for (const [marker, label] of [
    ['self.db.get_database_backend() == DatabaseBackend::Postgres', 'PostgreSQL read boundary'],
    ['list_categories_from_taxonomy(self, tenant_id, &locale).await', 'retained Taxonomy read'],
    ['list_categories_from_product_donor(self, tenant_id, &locale).await', 'retained non-PostgreSQL donor read'],
    ['FROM catalog_category_translations translation', 'non-PostgreSQL donor localized storage'],
    ['INSERT INTO catalog_category_seo_translations', 'Product-owned PostgreSQL SEO write'],
    ['sync_module_category_in_tx(', 'retained Taxonomy create sync'],
    ['category_legacy_translation_write_is_non_postgres_only', 'backend boundary unit regression'],
  ]) {
    need(categories, marker, label);
  }
  forbid(categories, 'product/category', 'duplicate Product Category Translation provider');

  for (const [marker, label] of [
    ['Status: **source-complete PostgreSQL legacy canonical write retirement; physical donor retirement pending**', 'CAT-28 bounded status'],
    ['PostgreSQL does **not** insert a new `catalog_category_translations` row', 'PostgreSQL legacy write retirement contract'],
    ['non-PostgreSQL backends still write `catalog_category_translations`', 'non-PostgreSQL write compatibility'],
    ['This is write retirement, not physical storage retirement', 'no premature physical drop'],
    ['No `product/category` Translation provider is introduced', 'no duplicate Translation provider'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-28 stops new PostgreSQL creates from writing the legacy canonical translation mirror', 'locale CAT-28 status'],
    ['`should_write_legacy_category_translation(DatabaseBackend::Postgres)` is false', 'locale backend boundary'],
    ['non-PostgreSQL backends continue to insert the legacy Product translation row', 'locale non-PostgreSQL compatibility'],
    ['CAT-28 does not delete or rewrite historical legacy rows', 'locale no premature drop'],
  ]) {
    need(localeContract, marker, label);
  }

  for (const [marker, label] of [
    ['PR #3739 / TAXONOMY-CAT-27', 'accepted CAT-27 history'],
    ['TAXONOMY-CAT-27 isolates those SEO fields', 'retained CAT-27 ownership marker'],
    ['TAXONOMY-CAT-28 now retires new PostgreSQL canonical mirror writes', 'CAT-28 plan cursor'],
    ['separate fail-closed donor-retirement slice', 'physical retirement sequencing'],
    ['Non-PostgreSQL donor compatibility remains explicit', 'backend compatibility sequencing'],
  ]) {
    need(plan, marker, label);
  }

  for (const [marker, label] of [
    ['Assert bounded CAT-27 file set when CAT-27 changes', 'progression-safe CAT-27 scope step'],
    ['CAT-27 migration/verifier unchanged; PR-wide CAT-27 slice restriction is not applicable.', 'CAT-27 scope skip marker'],
  ]) {
    need(retainedSeoWorkflow, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-category-legacy-write-retirement] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-category-legacy-write-retirement] PostgreSQL legacy Category canonical writes retired');
