#!/usr/bin/env node

import fs from 'node:fs';

const categoriesPath =
  'crates/rustok-product/src/services/catalog_schema_service/categories.rs';
const readPortPath = 'crates/rustok-product/src/catalog_schema_read_port.rs';
const contractPath = 'crates/rustok-product/docs/category-taxonomy-binding.md';
const localeContractPath = 'crates/rustok-product/docs/category-locale-contract.md';

const failures = [];
const need = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}: ${marker}`);
};
const compact = (source) => source.replace(/\s+/g, ' ').trim();

for (const path of [categoriesPath, readPortPath, contractPath, localeContractPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const categories = fs.readFileSync(categoriesPath, 'utf8');
  const readPort = compact(fs.readFileSync(readPortPath, 'utf8'));
  const contract = compact(fs.readFileSync(contractPath, 'utf8'));
  const localeContract = compact(fs.readFileSync(localeContractPath, 'utf8'));

  need(
    readPort,
    'Optional Product-owned read boundary for catalog schema directory',
    'schema-directory read-port boundary',
  );
  need(
    readPort,
    'const LIST_CATEGORIES_OPERATION: &str = "list_catalog_categories"',
    'bounded Product Category schema-directory operation',
  );

  const composeStart = categories.indexOf('fn compose_taxonomy_category_list_records(');
  const donorStart = categories.indexOf('async fn list_categories_from_product_donor(');
  if (composeStart < 0 || donorStart <= composeStart) {
    failures.push('Taxonomy composition and donor-list helper boundaries are required');
  } else {
    const taxonomyComposition = categories.slice(composeStart, donorStart);
    for (const [marker, label] of [
      ['taxonomy_category_hierarchy_order(&owners)?', 'Taxonomy hierarchy order materialization'],
      ['rows.sort_by_key(', 'final PostgreSQL schema-directory reorder'],
      ['owner.position < 0', 'invalid canonical sibling position fail-closed'],
      ['children_by_parent', 'canonical parent/child traversal'],
      ['left.position', 'canonical sibling position ordering'],
      ['left.canonical_key', 'deterministic canonical-key sibling tie-break'],
      ['contains a cycle', 'cyclic owner projection fail-closed'],
      ['path: row.path', 'retained Product path projection'],
      ['parent_id: owner.parent_id', 'retained Taxonomy canonical parent composition'],
    ]) {
      need(taxonomyComposition, marker, label);
    }
  }

  for (const [marker, label] of [
    [
      'category_taxonomy_directory_order_uses_owner_hierarchy_not_product_path',
      'Taxonomy schema-directory order regression',
    ],
    [
      'category_taxonomy_directory_order_fails_closed_on_invalid_owner_ordering',
      'Taxonomy order fail-closed regression',
    ],
    ['ORDER BY c.path ASC', 'retained non-PostgreSQL Product path ordering'],
  ]) {
    need(categories, marker, label);
  }

  for (const [source, marker, label] of [
    [contract, 'TAXONOMY-CAT-31', 'CAT-31 Product migration cursor'],
    [contract, 'schema-directory ordering', 'CAT-31 schema-directory ownership statement'],
    [
      contract,
      'Product `path` remains a Product-owned navigation projection',
      'retained Product navigation/path ownership',
    ],
    [localeContract, 'TAXONOMY-CAT-31', 'CAT-31 locale/read contract marker'],
    [
      localeContract,
      'Taxonomy parent/position hierarchy order',
      'Taxonomy canonical directory-order contract',
    ],
    [
      localeContract,
      'non-PostgreSQL backends retain Product `path` ordering',
      'cross-backend Product path ordering compatibility',
    ],
  ]) {
    need(source, marker, label);
  }
}

if (failures.length > 0) {
  console.error('[product-taxonomy-category-directory-order] verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  '[product-taxonomy-category-directory-order] PostgreSQL schema-directory Taxonomy hierarchy ordering verified',
);
