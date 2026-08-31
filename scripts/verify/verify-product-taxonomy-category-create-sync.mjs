#!/usr/bin/env node

import fs from 'node:fs';

const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const ownerSyncPath = 'crates/rustok-taxonomy/src/owner_category_sync.rs';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [categoriesPath, contractPath, localeContractPath, ownerSyncPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(fs.readFileSync(localeContractPath, 'utf8'));
  const ownerSync = fs.readFileSync(ownerSyncPath, 'utf8');

  const createStart = categories.indexOf('pub async fn create_category(');
  const listStart = categories.indexOf('pub async fn list_categories(');
  if (createStart < 0 || listStart <= createStart) {
    failures.push('Product create/list Category function boundaries are required');
  } else {
    const createBody = categories.slice(createStart, listStart);
    const donorTranslation = createBody.indexOf('INSERT INTO catalog_category_translations');
    const taxonomySync = createBody.indexOf('sync_created_category_to_taxonomy_in_tx(');
    const domainEvent = createBody.indexOf('DomainEvent::CatalogCategoryCreated');
    const commit = createBody.indexOf('txn.commit().await?');

    for (const [index, label] of [
      [donorTranslation, 'Product localized donor write'],
      [taxonomySync, 'Taxonomy owner-sync call'],
      [domainEvent, 'Product Category domain event'],
      [commit, 'shared Product transaction commit'],
    ]) {
      if (index < 0) failures.push(`missing create ordering marker: ${label}`);
    }
    if (
      donorTranslation >= 0 &&
      taxonomySync >= 0 &&
      domainEvent >= 0 &&
      commit >= 0 &&
      !(donorTranslation < taxonomySync && taxonomySync < domainEvent && domainEvent < commit)
    ) {
      failures.push(
        'Product create must write donor copy, synchronize Taxonomy, publish the domain event, then commit',
      );
    }

    const listBody = categories.slice(listStart);
    need(listBody, 'FROM catalog_categories c', 'retained Product donor Category fallback');
    need(
      listBody,
      'FROM catalog_category_translations translation',
      'retained Product donor localized fallback',
    );
  }

  for (const [marker, label] of [
    ['const PRODUCT_TAXONOMY_SCOPE: &str = "product";', 'Product Taxonomy module scope'],
    ['sync_module_category_in_tx(', 'transaction-bound Taxonomy owner sync'],
    ['for translation in translations', 'all normalized locales synchronized'],
    ['canonical_key: canonical_key_for_product_category(category_id)', 'same-ID canonical key'],
    ['slug: input.slug.clone()', 'single Product base slug projection'],
    ['parent_id: input.parent_id', 'Product parent projection'],
    ['position: input.position', 'Product sibling position projection'],
    ['aliases: Vec::new()', 'no fabricated route aliases'],
    ['INSERT INTO product_catalog_category_taxonomy_bindings', 'same-ID Product binding insert'],
    ['VALUES ($1, $2, $2, CURRENT_TIMESTAMP)', 'same-ID binding identity'],
    ['TaxonomyError::Database(error) => CommerceError::Database(error)', 'database error preservation'],
    ['Product Category Taxonomy synchronization failed:', 'domain conflict mapping'],
    ['normalize_term_route_key(slug)', 'canonical Taxonomy route validation'],
    ['TAXONOMY_CATEGORY_ROUTE_KEY_MAX_BYTES: usize = 120', 'Taxonomy route bound'],
    ['if position < 0', 'Taxonomy hierarchy position bound'],
    ['name.chars().count() > 120', 'Taxonomy canonical name bound'],
    ['.map(str::trim)', 'canonical description trimming'],
    ['value.chars().count() > 2_000', 'Taxonomy canonical description bound'],
    ['category_taxonomy_create_normalizes_canonical_copy', 'canonical-copy unit regression'],
    ['category_taxonomy_create_rejects_incompatible_canonical_input', 'canonical-input unit regression'],
  ]) {
    need(categories, marker, label);
  }
  if (categories.includes('product/category')) {
    failures.push('forbidden duplicate Product Category Translation provider: product/category');
  }

  for (const [marker, label] of [
    ['pub async fn sync_module_category_in_tx(', 'existing Taxonomy owner-sync public port'],
    ['The operation is idempotent and runs inside the caller transaction', 'owner-sync transaction contract'],
    ['ensure_route_key_available_in_tx(', 'Taxonomy route authority'],
    ['serialize_category_hierarchy_writer(txn, tenant_id).await?', 'Taxonomy hierarchy writer serialization'],
  ]) {
    need(ownerSync, marker, label);
  }

  for (const [marker, label] of [
    ['Status: **source-complete create dual-write; Product read cutover and donor retirement pending**', 'CAT-25 bounded status'],
    ['every new Product Category create now mirrors canonical identity', 'post-backfill create gap closure'],
    ['only then may `CatalogCategoryCreated`', 'event-after-owner-sync contract'],
    ['rolls back the Product donor inserts as part of that same transaction', 'atomic failure contract'],
    ['CAT-25 does **not** switch Product reads', 'historical no-read-cutover boundary'],
    ['`TaxonomyOwnerCategoryReader`', 'declared subsequent read projection'],
    ['No `product/category` Translation provider is introduced', 'no duplicate Translation provider'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-25 now dual-writes new Category creates', 'locale contract create dual-write'],
    ['1..120 characters', 'locale contract canonical name bound'],
    ['blank descriptions become `None`', 'locale contract canonical description normalization'],
    ['same-ID Product ↔ Taxonomy binding is inserted only after every locale succeeds', 'locale contract binding-last order'],
    ['`ProductCatalogSchemaService::list_categories` still reads the Product donor', 'historical donor read retention'],
    ['create dual-write is the prerequisite', 'read cutover sequencing'],
  ]) {
    need(localeContract, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-create-sync] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-taxonomy-category-create-sync] atomic Product Category create dual-write verified');
