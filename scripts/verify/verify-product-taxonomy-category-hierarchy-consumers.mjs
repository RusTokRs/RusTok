#!/usr/bin/env node

import fs from 'node:fs';

const files = {
  effectiveForms:
    'crates/rustok-product/src/services/catalog_schema_service/effective_forms.rs',
  ownerRead: 'crates/rustok-taxonomy/src/owner_category_read.rs',
  contract: 'crates/rustok-product/docs/category-taxonomy-binding.md',
};

const failures = [];
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: ${marker}`);
};

for (const path of Object.values(files)) {
  if (!fs.existsSync(path)) failures.push(`required file missing: ${path}`);
}

if (failures.length === 0) {
  const effectiveForms = fs.readFileSync(files.effectiveForms, 'utf8');
  const ownerRead = fs.readFileSync(files.ownerRead, 'utf8');
  const contract = fs.readFileSync(files.contract, 'utf8').replace(/\s+/g, ' ');

  for (const [marker, label] of [
    ['pub async fn load_scoped_categories_in<C>(', 'generic Taxonomy owner read'],
    ['Self::load_scoped_categories_in(', 'existing reader delegates to generic seam'],
    ['C: ConnectionTrait', 'host connection boundary'],
  ]) {
    requireMarker(ownerRead, marker, label);
  }

  for (const [marker, label] of [
    ['self.db.get_database_backend() == DatabaseBackend::Postgres', 'PostgreSQL group-label cutover'],
    ['load_product_taxonomy_category_parent_map(&self.db, tenant_id).await?', 'group-label Taxonomy hierarchy'],
    ['taxonomy_ancestor_chain(category_id, &parent_map)?', 'root-to-leaf Taxonomy ancestry'],
    ['FROM catalog_category_closure', 'non-PostgreSQL closure compatibility'],
    ['Some(load_product_taxonomy_category_parent_map(db, tenant_id).await?)', 'effective-form Taxonomy hierarchy'],
    ['LEFT JOIN product_catalog_category_taxonomy_bindings binding', 'typed Product binding'],
    ['TaxonomyOwnerCategoryReader::load_scoped_categories_in(', 'transaction-compatible Taxonomy owner projection'],
    ['owner.parent_id', 'Taxonomy canonical parent projection'],
    ['missing its Taxonomy Category binding', 'missing binding fail closed'],
    ['bound to incompatible Taxonomy Category', 'same-ID binding fail closed'],
    ['incompatible Taxonomy canonical key', 'canonical-key fail closed'],
    ['incompatible Taxonomy scope', 'scope fail closed'],
    ['category_taxonomy_hierarchy_builds_root_to_leaf_ancestor_chain', 'ancestry unit regression'],
    ['category_taxonomy_hierarchy_fails_closed_on_missing_or_cyclic_owner_state', 'fail-closed ancestry regression'],
  ]) {
    requireMarker(effectiveForms, marker, label);
  }

  for (const [marker, label] of [
    ['TAXONOMY-CAT-30', 'CAT-30 contract'],
    ['effective Product form/schema resolution', 'Product schema inheritance boundary'],
    ['inherited category attribute-group labels', 'Product label ancestry boundary'],
    ['Product `path` and closure persistence', 'retained Product navigation compatibility'],
    ['non-PostgreSQL', 'cross-backend compatibility'],
  ]) {
    requireMarker(contract, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-hierarchy-consumers] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[product-taxonomy-category-hierarchy-consumers] contract verified');