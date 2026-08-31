#!/usr/bin/env node

import fs from 'node:fs';

const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const effectiveFormsPath =
  'crates/rustok-product/src/services/catalog_schema_service/effective_forms.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const retainedDirectoryWorkflowPath =
  '.github/workflows/product-taxonomy-category-directory-order.yml';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const normalizeWhitespace = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [
  categoriesPath,
  effectiveFormsPath,
  contractPath,
  retainedDirectoryWorkflowPath,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const effectiveForms = fs.readFileSync(effectiveFormsPath, 'utf8');
  const contract = normalizeWhitespace(fs.readFileSync(contractPath, 'utf8'));
  const retainedDirectoryWorkflow = fs.readFileSync(retainedDirectoryWorkflowPath, 'utf8');

  const createStart = categories.indexOf('pub async fn create_category(');
  const listStart = categories.indexOf('pub async fn list_categories(');
  if (createStart < 0 || listStart <= createStart) {
    failures.push('Product create/list Category function boundaries are required');
  } else {
    const createBody = categories.slice(createStart, listStart);
    const closureGuard = createBody.indexOf(
      'if should_write_product_category_closure(txn.get_database_backend())',
    );
    const selfClosure = createBody.indexOf(
      'INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth)',
    );
    const ancestorClosure = createBody.indexOf('FROM catalog_category_closure');
    const taxonomySync = createBody.indexOf('sync_created_category_to_taxonomy_in_tx(');
    const commit = createBody.indexOf('txn.commit().await?');

    for (const [index, label] of [
      [closureGuard, 'backend-bounded Product closure write guard'],
      [selfClosure, 'retained non-PostgreSQL self closure write'],
      [ancestorClosure, 'retained non-PostgreSQL ancestor closure write'],
      [taxonomySync, 'Taxonomy owner sync'],
      [commit, 'shared Product transaction commit'],
    ]) {
      if (index < 0) failures.push(`missing create marker: ${label}`);
    }
    if (
      closureGuard >= 0 &&
      selfClosure >= 0 &&
      ancestorClosure >= 0 &&
      !(closureGuard < selfClosure && selfClosure < ancestorClosure)
    ) {
      failures.push('Product closure writes must remain inside the backend compatibility guard');
    }
  }

  const closureHelperStart = categories.indexOf(
    'fn should_write_product_category_closure(',
  );
  const syncStart = categories.indexOf('async fn sync_created_category_to_taxonomy_in_tx(');
  if (closureHelperStart < 0 || syncStart <= closureHelperStart) {
    failures.push('closure write/Taxonomy sync helper boundaries are required');
  } else {
    const closureHelper = categories.slice(closureHelperStart, syncStart);
    need(
      closureHelper,
      'backend != DatabaseBackend::Postgres',
      'PostgreSQL Product closure write retirement',
    );
  }

  for (const [marker, label] of [
    ['category_closure_write_is_non_postgres_only', 'backend boundary unit regression'],
    ['DatabaseBackend::Postgres', 'PostgreSQL backend regression'],
    ['DatabaseBackend::Sqlite', 'SQLite compatibility regression'],
    ['DatabaseBackend::MySql', 'MySQL compatibility regression'],
  ]) {
    need(categories, marker, label);
  }

  const labelsStart = effectiveForms.indexOf('pub async fn load_effective_form_group_labels(');
  const categoryMapStart = effectiveForms.indexOf('async fn load_category_schema_map<C>(');
  if (labelsStart < 0 || categoryMapStart <= labelsStart) {
    failures.push('effective-form group label boundaries are required');
  } else {
    const labelsBody = effectiveForms.slice(labelsStart, categoryMapStart);
    const postgresBranch = labelsBody.indexOf(
      'self.db.get_database_backend() == DatabaseBackend::Postgres',
    );
    const taxonomyChain = labelsBody.indexOf('taxonomy_ancestor_chain(category_id, &parent_map)?');
    const compatibilityElse = labelsBody.indexOf('} else {');
    const closureRead = labelsBody.indexOf('FROM catalog_category_closure');
    if (
      postgresBranch < 0 ||
      taxonomyChain < 0 ||
      compatibilityElse < 0 ||
      closureRead < 0 ||
      !(postgresBranch < taxonomyChain && taxonomyChain < compatibilityElse && compatibilityElse < closureRead)
    ) {
      failures.push(
        'PostgreSQL must consume Taxonomy ancestry while Product closure remains only in the non-PostgreSQL compatibility branch',
      );
    }
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-32 PostgreSQL closure write retirement', 'CAT-32 migration cursor'],
    ['PostgreSQL no longer materializes new `catalog_category_closure` rows', 'PostgreSQL closure write retirement contract'],
    ['Non-PostgreSQL backends continue to materialize Product closure rows', 'non-PostgreSQL closure compatibility'],
    ['does **not** drop `catalog_category_closure`', 'no premature physical closure drop'],
    ['`parent_id`, `path` and `level` remain Product-owned projections', 'retained Product hierarchy/navigation projection'],
  ]) {
    need(contract, marker, label);
  }

  for (const [marker, label] of [
    ['Verify Product Category schema-directory order contract when CAT-31 changes', 'progression-safe CAT-31 verifier step'],
    ['CAT-31 runtime/verifier unchanged; historical CAT-31 source verifier is not applicable.', 'CAT-31 verifier skip marker'],
    ['CAT-31 runtime/verifier unchanged; PR-wide CAT-31 slice restriction is not applicable.', 'CAT-31 scope skip marker'],
  ]) {
    need(retainedDirectoryWorkflow, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-category-closure-write-retirement] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-category-closure-write-retirement] PostgreSQL Product Category closure writes retired');
