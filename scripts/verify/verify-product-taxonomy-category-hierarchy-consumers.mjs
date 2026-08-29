#!/usr/bin/env node

import fs from 'node:fs';

const effectiveFormsPath =
  'crates/rustok-product/src/services/catalog_schema_service/effective_forms.rs';
const ownerReadPath = 'crates/rustok-taxonomy/src/owner_category_read.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const planPath = 'crates/rustok-taxonomy/docs/implementation-plan.md';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const forbid = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`forbidden ${label}: ${marker}`);
};
const compact = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [effectiveFormsPath, ownerReadPath, contractPath, planPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const effectiveForms = fs.readFileSync(effectiveFormsPath, 'utf8');
  const ownerRead = fs.readFileSync(ownerReadPath, 'utf8');
  const contract = compact(fs.readFileSync(contractPath, 'utf8'));
  const plan = compact(fs.readFileSync(planPath, 'utf8'));

  for (const [marker, label] of [
    ['pub async fn load_scoped_categories_in<C>(', 'transaction-compatible Taxonomy owner read'],
    ['C: ConnectionTrait', 'generic host connection boundary'],
    ['Self::load_scoped_categories_in(', 'existing owner reader delegation'],
    ['materialize_categories(', 'retained Taxonomy-owned composition'],
  ]) {
    need(ownerRead, marker, label);
  }

  const labelsStart = effectiveForms.indexOf('pub async fn load_effective_form_group_labels(');
  const schemaMapStart = effectiveForms.indexOf('async fn load_category_schema_map<C>(');
  if (labelsStart < 0 || schemaMapStart <= labelsStart) {
    failures.push('effective-form group-label/schema-map boundaries are required');
  } else {
    const labelsBody = effectiveForms.slice(labelsStart, schemaMapStart);
    need(
      labelsBody,
      'self.db.get_database_backend() == DatabaseBackend::Postgres',
      'PostgreSQL-only hierarchy consumer cutover',
    );
    need(
      labelsBody,
      'load_product_taxonomy_category_parent_map(&self.db, tenant_id).await?',
      'Taxonomy parent projection for group labels',
    );
    need(
      labelsBody,
      'taxonomy_ancestor_chain(category_id, &parent_map)?',
      'Taxonomy ancestry traversal',
    );
    need(
      labelsBody,
      'FROM catalog_category_closure',
      'non-PostgreSQL closure compatibility fallback',
    );
  }

  const attributeMapStart = effectiveForms.indexOf('async fn load_attribute_schema_map<C>(');
  if (schemaMapStart < 0 || attributeMapStart <= schemaMapStart) {
    failures.push('category-schema/attribute-schema helper boundaries are required');
  } else {
    const schemaMapBody = effectiveForms.slice(schemaMapStart, attributeMapStart);
    need(
      schemaMapBody,
      'db.get_database_backend() == DatabaseBackend::Postgres',
      'PostgreSQL schema ancestry cutover',
    );
    need(
      schemaMapBody,
      'Some(load_product_taxonomy_category_parent_map(db, tenant_id).await?)',
      'Taxonomy parent projection for effective schema',
    );
    need(
      schemaMapBody,
      'parent_category_id,',
      'owner-projected parent passed into Product schema resolver',
    );
  }

  const hierarchyStart = effectiveForms.indexOf('async fn load_product_taxonomy_category_parent_map<C>(');
  const ancestryStart = effectiveForms.indexOf('fn taxonomy_ancestor_chain(');
  if (hierarchyStart < 0 || ancestryStart <= hierarchyStart) {
    failures.push('Taxonomy hierarchy projection/ancestry helpers are required');
  } else {
    const hierarchyBody = effectiveForms.slice(hierarchyStart, ancestryStart);
    for (const [marker, label] of [
      ['LEFT JOIN product_catalog_category_taxonomy_bindings binding', 'typed Product binding read'],
      ['missing its Taxonomy Category binding', 'missing binding fail-closed'],
      ['bound to incompatible Taxonomy Category', 'same-ID binding fail-closed'],
      ['TaxonomyOwnerCategoryReader::load_scoped_categories_in(', 'transaction-compatible owner read use'],
      ['TaxonomyScopeType::Module', 'module scope filter'],
      ['Some(PRODUCT_TAXONOMY_SCOPE)', 'Product scope filter'],
      ['Some(&category_ids)', 'bounded Product Category IDs'],
      ['owner.parent_id', 'Taxonomy canonical hierarchy projection'],
      ['incompatible Taxonomy canonical key', 'canonical-key fail-closed'],
      ['incompatible Taxonomy scope', 'scope fail-closed'],
    ]) {
      need(hierarchyBody, marker, label);
    }
    forbid(
      hierarchyBody,
      'catalog_category_closure',
      'Product closure as PostgreSQL canonical ancestry source',
    );
    forbid(
      hierarchyBody,
      'c.parent_id',
      'Product parent_id as PostgreSQL canonical ancestry source',
    );
  }

  for (const [marker, label] of [
    ['category_taxonomy_hierarchy_builds_root_to_leaf_ancestor_chain', 'Taxonomy ancestry unit regression'],
    ['category_taxonomy_hierarchy_fails_closed_on_missing_or_cyclic_owner_state', 'fail-closed ancestry unit regression'],
    ['Product Category Taxonomy hierarchy projection failed:', 'Taxonomy hierarchy error mapping'],
  ]) {
    need(effectiveForms, marker, label);
  }

  need(contract, 'TAXONOMY-CAT-30', 'CAT-30 Product contract marker');
  need(plan, 'TAXONOMY-CAT-30', 'CAT-30 central plan marker');
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-hierarchy-consumers] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[product-taxonomy-category-hierarchy-consumers] PostgreSQL Product Category hierarchy consumers verified',
);