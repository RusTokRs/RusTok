#!/usr/bin/env node

import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const failures = [];
const requireMarker = (path, marker, label = marker) => {
  const source = read(path);
  if (!source.includes(marker)) failures.push(`${path}: missing ${label}`);
};
const rejectMarker = (path, marker, label = marker) => {
  const source = read(path);
  if (source.includes(marker)) failures.push(`${path}: must not contain ${label}`);
};

const migration = 'crates/rustok-forum/src/migrations/m20260823_000029_add_forum_taxonomy_category_binding.rs';
const migrationRegistry = 'crates/rustok-forum/src/migrations/mod.rs';
const backfillContracts = 'docs/migrations/backfill-contracts.json';
const relation = 'crates/rustok-forum/src/entities/forum_category_taxonomy_binding.rs';
const runtimeTest = 'crates/rustok-forum/tests/category_taxonomy_binding.rs';
const entities = 'crates/rustok-forum/src/entities/mod.rs';
const legacyCategory = 'crates/rustok-forum/src/entities/forum_category.rs';
const categoryService = 'crates/rustok-forum/src/services/category.rs';
const categoryMutationSupport = 'crates/rustok-forum/src/services/category_mutation_support.rs';
const categoryImport = 'crates/rustok-forum/src/services/category_import.rs';
const categoryProjectionOwner = 'crates/rustok-forum/src/services/category_projection_owner.rs';
const forumServices = 'crates/rustok-forum/src/services/mod.rs';

for (const path of [
  migration,
  migrationRegistry,
  backfillContracts,
  relation,
  runtimeTest,
  entities,
  legacyCategory,
  categoryService,
  categoryMutationSupport,
  categoryImport,
  categoryProjectionOwner,
  forumServices,
]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  requireMarker(migration, 'forum_category_taxonomy_bindings', 'typed Forum→Taxonomy binding table');
  requireMarker(migration, 'fk_forum_category_taxonomy_binding_forum', 'Forum composite foreign key');
  requireMarker(migration, 'fk_forum_category_taxonomy_binding_taxonomy', 'Taxonomy composite foreign key');
  requireMarker(migration, 'uq_forum_category_taxonomy_binding_taxonomy', 'one-to-one tenant binding index');
  requireMarker(migration, '(TaxonomyTerms::TenantId, TaxonomyTerms::Id)', 'tenant-safe Taxonomy identity target');
  requireMarker(migrationRegistry, 'm20260823_000029_add_forum_taxonomy_category_binding', 'registered CAT-5 migration');
  requireMarker(migrationRegistry, 'm20260711_000001_add_tenant_identity_key', 'Taxonomy tenant identity dependency');
  requireMarker(backfillContracts, 'forum-taxonomy-category-binding-bootstrap', 'CAT-5 backfill declaration');
  requireMarker(backfillContracts, '"migration": "m20260823_000029_add_forum_taxonomy_category_binding"', 'CAT-5 migration backfill registration');
  requireMarker(backfillContracts, '"mode": "none"', 'empty binding-table backfill mode');

  requireMarker(relation, 'ForumCategoryTaxonomyBindingService', 'bounded binding service');
  requireMarker(relation, 'taxonomy_term_identity_exists', 'Taxonomy owner identity validation');
  requireMarker(relation, 'TaxonomyTermKind::Category', 'Category-only owner validation');
  requireMarker(relation, 'same-tenant Taxonomy Category', 'cross-tenant fail-closed contract');
  requireMarker(relation, 'already bound to a different Taxonomy Category', 'no implicit rebind contract');
  requireMarker(relation, 'already bound to another Forum category', 'one-to-one duplicate guard');
  requireMarker(entities, 'pub mod forum_category_taxonomy_binding;', 'binding entity registration');

  requireMarker(runtimeTest, 'forum_category_binding_is_category_only_tenant_bounded_and_one_to_one', 'runtime binding contract');
  requireMarker(runtimeTest, 'repeating the same binding should be idempotent', 'idempotent bind proof');
  requireMarker(runtimeTest, 'Taxonomy Tags must not masquerade as Categories', 'wrong-kind runtime proof');
  requireMarker(runtimeTest, 'foreign-tenant Taxonomy Categories must fail closed', 'foreign-tenant runtime proof');
  requireMarker(runtimeTest, 'stale Taxonomy Category identities must fail closed', 'stale identity runtime proof');

  rejectMarker(legacyCategory, 'taxonomy_category_id', 'binding state embedded in legacy category row');
  requireMarker(legacyCategory, 'pub parent_id: Option<Uuid>', 'legacy hierarchy retained during staged cutover');

  requireMarker(categoryService, 'include!("category_mutation_support.rs");', 'explicit shared Category mutation support');
  requireMarker(categoryService, 'pub(super) struct CategoryService;', 'crate-private Category persistence seam');
  requireMarker(categoryService, 'pub(crate) async fn ensure_exists_in_tx(', 'retained Category existence helper');
  requireMarker(categoryService, 'pub(crate) async fn find_category_in_tx(', 'retained Category lookup helper');
  requireMarker(categoryService, 'pub(crate) async fn adjust_counters_in_tx(', 'retained Category counter helper');
  for (const [marker, label] of [
    ['async fn lock_category_tree_in_tx', 'shared Category tree-lock implementation'],
    ['async fn shift_siblings_for_insert_in_tx', 'shared Category insert-order implementation'],
    ['fn validate_category_name', 'shared Category name validation implementation'],
    ['fn normalize_locale(', 'shared Category locale normalization implementation'],
    ['fn normalize_required_slug', 'shared Category required-slug implementation'],
    ['fn normalize_slug(', 'shared Category slug normalization implementation'],
    ['use crate::dto::{CreateCategoryInput, UpdateCategoryInput};', 'Category command DTO imports'],
    ['use rustok_api::{Action, Resource};', 'Category command authorization imports'],
  ]) {
    rejectMarker(categoryService, marker, label);
  }

  requireMarker(categoryMutationSupport, 'Shared implementation support for the `category` include group', 'live shared-support boundary');
  requireMarker(categoryMutationSupport, 'use tracing::instrument;', 'projection owner instrumentation import');
  requireMarker(categoryMutationSupport, 'use crate::dto::{CreateCategoryInput, UpdateCategoryInput};', 'projection owner command DTO imports');
  requireMarker(categoryMutationSupport, 'use rustok_api::{Action, Resource};', 'projection owner authorization imports');
  requireMarker(categoryMutationSupport, 'async fn lock_category_tree_in_tx', 'shared Category tree lock');
  requireMarker(categoryMutationSupport, 'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))', 'tenant Category tree lock key');
  requireMarker(categoryMutationSupport, 'async fn shift_siblings_for_insert_in_tx', 'shared Category insert ordering');
  requireMarker(categoryMutationSupport, 'fn validate_category_name', 'shared Category name validator');
  requireMarker(categoryMutationSupport, 'fn normalize_locale(', 'shared Category locale normalizer');
  requireMarker(categoryMutationSupport, 'fn normalize_required_slug', 'shared Category required-slug normalizer');
  requireMarker(categoryMutationSupport, 'fn normalize_slug(', 'shared Category slug normalizer');
  rejectMarker(categoryMutationSupport, 'forum_category_translation', 'retired Forum-local Category translation donor');

  for (const marker of [
    'insert_import_category_in_tx(',
    'validate_category_name(&record.name)',
    'normalize_locale(&record.locale)',
    'normalize_required_slug(&record.slug)',
    'lock_category_tree_in_tx(txn, tenant_id)',
    'shift_siblings_for_insert_in_tx(',
    'taxonomy_sync::sync_category_copy_in_tx(',
  ]) {
    requireMarker(categoryImport, marker, `live import support call ${marker}`);
  }

  for (const marker of [
    'CategoryProjectionOwnerService',
    'enforce_scope(&security, Resource::ForumCategories, Action::Create)',
    'enforce_scope(&security, Resource::ForumCategories, Action::Update)',
    'lock_category_tree_in_tx(&txn, tenant_id)',
    'shift_siblings_for_insert_in_tx(',
    'validate_category_name(',
    'normalize_locale(',
    'normalize_required_slug',
    'taxonomy_sync::sync_category_copy_in_tx(',
  ]) {
    requireMarker(categoryProjectionOwner, marker, `live projection-owner support call ${marker}`);
  }

  rejectMarker(forumServices, 'ForumCategoryTranslationTargetProvider', 'retired duplicate Forum Translation provider');
}

if (failures.length > 0) {
  console.error('[forum-taxonomy-category-binding] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[forum-taxonomy-category-binding] typed staged binding boundary verified');
