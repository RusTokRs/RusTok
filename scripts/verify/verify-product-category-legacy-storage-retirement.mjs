#!/usr/bin/env node

import fs from 'node:fs';

const migrationPath =
  'crates/rustok-product/src/migrations/m20260829_000018_retire_product_category_legacy_translations.rs';
const migrationsModPath = 'crates/rustok-product/src/migrations/mod.rs';
const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';
const planPath = 'crates/rustok-taxonomy/docs/implementation-plan.md';
const databaseDocPath = 'docs/architecture/database.md';
const backfillContractsPath = 'docs/migrations/backfill-contracts.json';
const retainedWriteWorkflowPath =
  '.github/workflows/product-category-legacy-write-retirement.yml';
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
  migrationPath,
  migrationsModPath,
  categoriesPath,
  contractPath,
  localeContractPath,
  planPath,
  databaseDocPath,
  backfillContractsPath,
  retainedWriteWorkflowPath,
  retainedSeoWorkflowPath,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const migration = fs.readFileSync(migrationPath, 'utf8');
  const migrationsMod = fs.readFileSync(migrationsModPath, 'utf8');
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = normalizeWhitespace(
    fs.readFileSync(localeContractPath, 'utf8'),
  );
  const plan = normalizeWhitespace(fs.readFileSync(planPath, 'utf8'));
  const databaseDoc = normalizeWhitespace(fs.readFileSync(databaseDocPath, 'utf8'));
  const backfillContracts = JSON.parse(
    fs.readFileSync(backfillContractsPath, 'utf8'),
  );
  const retainedWriteWorkflow = fs.readFileSync(retainedWriteWorkflowPath, 'utf8');
  const retainedSeoWorkflow = fs.readFileSync(retainedSeoWorkflowPath, 'utf8');

  for (const [marker, label] of [
    [
      'manager.get_database_backend() != DatabaseBackend::Postgres',
      'PostgreSQL-only retirement boundary',
    ],
    ['manager.get_connection().begin().await?', 'single retirement transaction'],
    [
      'ensure_complete_taxonomy_ownership(&txn).await?',
      'Taxonomy identity ownership preflight',
    ],
    [
      'ensure_complete_taxonomy_locale_ownership(&txn).await?',
      'Taxonomy locale coverage preflight',
    ],
    [
      'ensure_complete_product_seo_ownership(&txn).await?',
      'Product SEO ownership preflight',
    ],
    [
      'DROP TABLE IF EXISTS catalog_category_translations',
      'legacy Product Category translation drop',
    ],
    ['txn.commit().await?', 'retirement transaction commit'],
    [
      'LEFT JOIN product_catalog_category_taxonomy_bindings binding',
      'typed Product-to-Taxonomy binding preflight',
    ],
    [
      'binding.tenant_id = category.tenant_id',
      'tenant-safe binding ownership',
    ],
    [
      'binding.catalog_category_id = category.id',
      'Product Category binding identity',
    ],
    ['taxonomy_id != category.id', 'same-ID Taxonomy ownership guard'],
    ['term.tenant_id != category.tenant_id', 'same-tenant Taxonomy ownership guard'],
    [
      'term.kind != TaxonomyTermKind::Category',
      'Taxonomy Category kind ownership guard',
    ],
    [
      'term.scope_type != TaxonomyScopeType::Module',
      'Taxonomy module scope ownership guard',
    ],
    [
      'term.scope_value != PRODUCT_SCOPE_VALUE',
      'Taxonomy product scope ownership guard',
    ],
    [
      'format!("product-category-{}", category.id)',
      'canonical Product Category key ownership guard',
    ],
    [
      'LEFT JOIN taxonomy_term_translations taxonomy_copy',
      'Taxonomy localized copy coverage source',
    ],
    [
      'taxonomy_copy.tenant_id = category.tenant_id',
      'Taxonomy localized copy tenant identity',
    ],
    [
      'taxonomy_copy.term_id = category.id',
      'Taxonomy localized copy term identity',
    ],
    [
      'taxonomy_copy.locale = legacy.locale',
      'Taxonomy same-locale coverage identity',
    ],
    ['WHERE taxonomy_copy.term_id IS NULL', 'missing Taxonomy locale guard'],
    [
      'legacy localized row(s) have no same-locale Taxonomy canonical copy',
      'fail-closed Taxonomy locale coverage error',
    ],
    [
      'FROM catalog_category_translations legacy',
      'legacy Product Category evidence source',
    ],
    ['JOIN catalog_categories category', 'tenant identity source for legacy evidence'],
    [
      'LEFT JOIN catalog_category_seo_translations seo',
      'Product-owned SEO target preflight',
    ],
    ['seo.tenant_id = category.tenant_id', 'SEO tenant identity match'],
    ['seo.category_id = legacy.category_id', 'SEO Product Category identity match'],
    ['seo.locale = legacy.locale', 'SEO locale identity match'],
    [
      'legacy.meta_title IS NOT NULL OR legacy.meta_description IS NOT NULL',
      'SEO-only legacy evidence filter',
    ],
    ['seo.category_id IS NULL', 'missing SEO row guard'],
    [
      'seo.meta_title IS DISTINCT FROM legacy.meta_title',
      'exact meta title parity guard',
    ],
    [
      'seo.meta_description IS DISTINCT FROM legacy.meta_description',
      'exact meta description parity guard',
    ],
    [
      'localized SEO row(s) are missing or incompatible in catalog_category_seo_translations',
      'fail-closed SEO parity error',
    ],
    ['Intentionally irreversible on PostgreSQL', 'irreversible retirement boundary'],
  ]) {
    need(migration, marker, label);
  }

  const taxonomyPreflight = migration.indexOf(
    'ensure_complete_taxonomy_ownership(&txn).await?',
  );
  const localePreflight = migration.indexOf(
    'ensure_complete_taxonomy_locale_ownership(&txn).await?',
  );
  const seoPreflight = migration.indexOf(
    'ensure_complete_product_seo_ownership(&txn).await?',
  );
  const drop = migration.indexOf(
    'DROP TABLE IF EXISTS catalog_category_translations',
  );
  const commit = migration.indexOf('txn.commit().await?');
  if (
    taxonomyPreflight < 0 ||
    localePreflight < 0 ||
    seoPreflight < 0 ||
    drop < 0 ||
    commit < 0 ||
    !(
      taxonomyPreflight < localePreflight &&
      localePreflight < seoPreflight &&
      seoPreflight < drop &&
      drop < commit
    )
  ) {
    failures.push(
      'retirement must prove Taxonomy identity, prove same-locale Taxonomy copy, prove Product SEO parity, drop donor, then commit',
    );
  }

  forbid(migration, 'legacy.name', 'legacy canonical name equality check');
  forbid(
    migration,
    'legacy.description',
    'legacy canonical description equality check',
  );
  forbid(
    migration,
    'CREATE TABLE catalog_category_translations',
    'recreated retired donor in migration down path',
  );

  for (const [marker, label] of [
    [
      'mod m20260829_000018_retire_product_category_legacy_translations;',
      'CAT-29 migration module registration',
    ],
    [
      'Box::new(m20260829_000018_retire_product_category_legacy_translations::Migration)',
      'CAT-29 migration execution registration',
    ],
  ]) {
    need(migrationsMod, marker, label);
  }

  const retirementFixture = backfillContracts.contracts?.find(
    (entry) =>
      entry.migration ===
      'm20260829_000018_retire_product_category_legacy_translations',
  );
  if (!retirementFixture) {
    failures.push('CAT-29 migration backfill fixture contract is required');
  } else {
    if (retirementFixture.id !== 'product-category-legacy-storage-retirement') {
      failures.push('CAT-29 migration fixture must use the bounded Product retirement id');
    }
    if (retirementFixture.mode !== 'fixture') {
      failures.push('CAT-29 migration must use a PostgreSQL N-1 fixture contract');
    }
    if (retirementFixture.owner !== 'rustok-product') {
      failures.push('CAT-29 migration fixture must remain Product-owned');
    }
    for (const [marker, label] of [
      ['Historical donor category', 'stale donor canonical copy fixture'],
      ['Current Taxonomy category', 'independently evolved Taxonomy copy fixture'],
      ['product_catalog_category_taxonomy_bindings', 'same-ID binding fixture'],
      ['catalog_category_seo_translations', 'Product SEO fixture'],
      ['Preserved SEO title', 'exact Product SEO title fixture'],
      ['Preserved SEO description', 'exact Product SEO description fixture'],
    ]) {
      need(retirementFixture.setup_sql ?? '', marker, label);
    }
    for (const [marker, label] of [
      [
        "table_name = 'catalog_category_translations'",
        'physical donor drop assertion',
      ],
      ['Current Taxonomy category', 'Taxonomy copy survival assertion'],
      ['taxonomy_term_route_keys', 'Taxonomy route survival assertion'],
      ['product_catalog_category_taxonomy_bindings', 'binding survival assertion'],
      ['catalog_category_seo_translations', 'Product SEO survival assertion'],
      ['Preserved SEO title', 'Product SEO title survival assertion'],
      ['Preserved SEO description', 'Product SEO description survival assertion'],
    ]) {
      need(retirementFixture.assertion_sql ?? '', marker, label);
    }
  }

  for (const [marker, label] of [
    [
      'should_write_legacy_category_translation(txn.get_database_backend())',
      'retained CAT-28 PostgreSQL legacy-write boundary',
    ],
    [
      'list_categories_from_taxonomy(self, tenant_id, &locale).await',
      'retained PostgreSQL Taxonomy read projection',
    ],
    [
      'list_categories_from_product_donor(self, tenant_id, &locale).await',
      'retained non-PostgreSQL donor read projection',
    ],
    [
      'INSERT INTO catalog_category_seo_translations',
      'retained Product-owned PostgreSQL SEO write',
    ],
  ]) {
    need(categories, marker, label);
  }
  forbid(categories, 'product/category', 'duplicate Product Category Translation provider');

  for (const [marker, label] of [
    [
      'Status: **source-complete PostgreSQL legacy Category translation storage retirement; non-PostgreSQL donor compatibility retained**',
      'CAT-29 current status',
    ],
    [
      'Migration `m20260829_000018_retire_product_category_legacy_translations` is PostgreSQL-only',
      'CAT-29 PostgreSQL-only contract',
    ],
    [
      'every Product Category, including historical/soft-deleted Product rows, has a typed Product → Taxonomy binding for the same UUID',
      'complete Product Category Taxonomy ownership preflight',
    ],
    [
      'every historical legacy locale still has a same-tenant, same-ID Taxonomy localized row for that exact locale',
      'complete Taxonomy locale coverage contract',
    ],
    [
      '`(tenant_id, category_id, locale)` row in `catalog_category_seo_translations`',
      'exact Product SEO identity contract',
    ],
    [
      'legacy `name` / `description` are deliberately **not** compared byte-for-byte with current Taxonomy copy',
      'no stale canonical donor equality requirement',
    ],
    [
      '`DROP TABLE IF EXISTS catalog_category_translations` and commit',
      'transactional donor drop contract',
    ],
    [
      'non-PostgreSQL `catalog_category_translations` donor storage',
      'non-PostgreSQL donor compatibility contract',
    ],
    [
      'No `product/category` Translation provider is introduced',
      'no duplicate Product Category Translation provider',
    ],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    [
      'TAXONOMY-CAT-29 physically retires that mirror on PostgreSQL after fail-closed ownership checks',
      'CAT-29 locale status',
    ],
    [
      'every historical donor locale has a Taxonomy localized row for the exact same tenant/category/locale identity',
      'locale Taxonomy coverage contract',
    ],
    [
      'exact normalized `(tenant_id, category_id, locale)` identity',
      'locale SEO identity contract',
    ],
    [
      'uses `IS DISTINCT FROM` for both SEO fields',
      'locale SEO parity contract',
    ],
    [
      'does not compare historical legacy `name` or `description` bytes with current Taxonomy copy',
      'locale no stale canonical equality requirement',
    ],
    [
      'On SQLite/MySQL and other non-PostgreSQL backends CAT-29 is a no-op',
      'locale non-PostgreSQL compatibility',
    ],
  ]) {
    need(localeContract, marker, label);
  }

  for (const [marker, label] of [
    ['PR #3740 / TAXONOMY-CAT-28', 'accepted CAT-28 history'],
    [
      'TAXONOMY-CAT-29 is the bounded PostgreSQL physical donor-retirement slice',
      'CAT-29 plan cursor',
    ],
    [
      'expected `product-category-{uuid}` canonical key',
      'plan canonical-key ownership guard',
    ],
    [
      'exact Product-owned SEO parity',
      'plan Product SEO parity guard',
    ],
    [
      'Non-PostgreSQL backends remain no-op and keep their donor storage/read/write path',
      'plan non-PostgreSQL compatibility',
    ],
    [
      'then audit the remaining Product-owned Category projections and policy surfaces',
      'post-retirement Product cursor',
    ],
  ]) {
    need(plan, marker, label);
  }

  for (const [marker, label] of [
    [
      '`catalog_categories`, `catalog_category_closure`',
      'Product Category live structural baseline',
    ],
    [
      '`catalog_category_seo_translations` for Product-owned localized Category SEO on PostgreSQL',
      'Product Category SEO live baseline',
    ],
    [
      'Taxonomy-owned Category identity, localized `name`/`slug`/`description`, routes and hierarchy',
      'Taxonomy canonical Product Category baseline',
    ],
    [
      'irreversibly drops the historical `catalog_category_translations` donor',
      'database schema retirement summary',
    ],
    [
      'Non-PostgreSQL backends retain the legacy Product Category translation donor/read/write path',
      'database non-PostgreSQL compatibility summary',
    ],
  ]) {
    need(databaseDoc, marker, label);
  }

  for (const [source, marker, label] of [
    [
      retainedWriteWorkflow,
      'Verify Product Category legacy write retirement contract when CAT-28 changes',
      'progression-safe retained CAT-28 verifier step',
    ],
    [
      retainedWriteWorkflow,
      'CAT-28 runtime/verifier unchanged; historical CAT-28 source verifier is not applicable.',
      'CAT-28 verifier skip marker',
    ],
    [
      retainedWriteWorkflow,
      'CAT-28 runtime/verifier unchanged; PR-wide CAT-28 slice restriction is not applicable.',
      'CAT-28 scope skip marker',
    ],
    [
      retainedSeoWorkflow,
      'Verify Product Category SEO seam contract when CAT-27 changes',
      'progression-safe retained CAT-27 verifier step',
    ],
    [
      retainedSeoWorkflow,
      'CAT-27 migration/verifier unchanged; historical CAT-27 source verifier is not applicable.',
      'CAT-27 verifier skip marker',
    ],
    [
      retainedSeoWorkflow,
      'CAT-27 migration/verifier unchanged; PR-wide CAT-27 slice restriction is not applicable.',
      'CAT-27 scope skip marker',
    ],
  ]) {
    need(source, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-category-legacy-storage-retirement] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[product-category-legacy-storage-retirement] PostgreSQL legacy Product Category translation storage retirement verified',
);
