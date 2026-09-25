#!/usr/bin/env node

import fs from 'node:fs';

const read = (file) => fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
const failures = [];
const requireMarker = (file, marker, label = marker) => {
  const source = read(file);
  if (!source.includes(marker)) failures.push(`${file}: missing ${label}`);
};
const rejectMarker = (file, marker, label = marker) => {
  const source = read(file);
  if (source.includes(marker)) failures.push(`${file}: must not contain ${label}`);
};

const migration = 'crates/modules/rustok-forum/src/migrations/m20260823_000029_add_forum_taxonomy_category_binding.rs';
const migrationRegistry = 'crates/modules/rustok-forum/src/migrations/mod.rs';
const backfillContracts = 'docs/migrations/backfill-contracts.json';
const relation = 'crates/modules/rustok-forum/src/entities/forum_category_taxonomy_binding.rs';
const runtimeTest = 'crates/modules/rustok-forum/tests/category_taxonomy_binding.rs';
const entities = 'crates/modules/rustok-forum/src/entities/mod.rs';
const legacyCategory = 'crates/modules/rustok-forum/src/entities/forum_category.rs';
const categoryService = 'crates/modules/rustok-forum/src/services/category.rs';
const categoryMutationSupport = 'crates/modules/rustok-forum/src/services/category_mutation_support.rs';
const categoryImport = 'crates/modules/rustok-forum/src/services/category_import.rs';
const categoryProjectionOwner = 'crates/modules/rustok-forum/src/services/category_projection_owner.rs';
const categoryCommandOwner = 'crates/modules/rustok-forum/src/services/category_command_owner.rs';
const categoryCommandLegacy = 'crates/modules/rustok-forum/src/services/category_command.rs';
const categoryPresentationLegacy = 'crates/modules/rustok-forum/src/category_presentation.rs';
const categoryLifecycle = 'crates/modules/rustok-forum/src/services/category_lifecycle.rs';
const forumServices = 'crates/modules/rustok-forum/src/services/mod.rs';
const forumLib = 'crates/modules/rustok-forum/src/lib.rs';
const forumManifest = 'crates/modules/rustok-forum/rustok-module.toml';
const forumComposition = 'modules.toml';
const taxonomyHierarchy = 'crates/modules/rustok-taxonomy/src/owner_category_hierarchy_mutation.rs';
const taxonomyRead = 'crates/modules/rustok-taxonomy/src/owner_category_read.rs';
const taxonomyLib = 'crates/modules/rustok-taxonomy/src/lib.rs';

for (const file of [
  migration, migrationRegistry, backfillContracts, relation, runtimeTest, entities,
  legacyCategory, categoryService, categoryMutationSupport, categoryImport,
  categoryProjectionOwner, categoryCommandOwner, categoryLifecycle, forumServices,
  forumLib, forumManifest, forumComposition, taxonomyHierarchy, taxonomyRead, taxonomyLib,
]) {
  if (!fs.existsSync(file)) failures.push(`${file}: file is required`);
}

if (failures.length === 0) {
  requireMarker(migration, 'forum_category_taxonomy_bindings', 'typed Forum→Taxonomy binding table');
  requireMarker(migration, 'fk_forum_category_taxonomy_binding_forum', 'Forum composite foreign key');
  requireMarker(migration, 'fk_forum_category_taxonomy_binding_taxonomy', 'Taxonomy composite foreign key');
  requireMarker(migrationRegistry, 'm20260823_000029_add_forum_taxonomy_category_binding', 'CAT-5 migration registration');
  requireMarker(backfillContracts, 'forum-taxonomy-category-binding-bootstrap', 'CAT-5 backfill declaration');
  requireMarker(relation, 'ForumCategoryTaxonomyBindingService', 'typed binding owner');
  requireMarker(runtimeTest, 'forum_category_binding_is_category_only_tenant_bounded_and_one_to_one', 'runtime binding contract');
  rejectMarker(legacyCategory, 'taxonomy_category_id', 'retired Taxonomy identity column in Forum Category');
  requireMarker(legacyCategory, 'pub parent_id: Option<Uuid>', 'legacy hierarchy placeholder retained only where explicitly staged');

  requireMarker(categoryService, 'pub(super) struct CategoryService;', 'Forum Category persistence seam');
  requireMarker(categoryService, 'pub(crate) async fn load_categories_in_tx(', 'bounded Forum Category loader');
  rejectMarker(categoryService, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  rejectMarker(categoryMutationSupport, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  requireMarker(categoryMutationSupport, 'fn validate_category_name', 'Forum-local Category validation');
  requireMarker(categoryMutationSupport, 'fn normalize_required_slug', 'Forum-local slug validation');

  requireMarker(categoryImport, 'taxonomy_sync::shift_category_siblings_for_insert_in_tx(', 'Taxonomy-owned insertion shift');
  rejectMarker(categoryImport, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  requireMarker(categoryProjectionOwner, 'taxonomy_sync::load_category_owner_snapshot_in_tx', 'Taxonomy owner projection');
  requireMarker(categoryProjectionOwner, 'taxonomy_sync::sync_category_copy_in_tx(', 'Taxonomy owner sync');
  requireMarker(categoryProjectionOwner, 'rustok_taxonomy::lock_category_hierarchy_writer_in_tx', 'canonical hierarchy lock');
  rejectMarker(categoryProjectionOwner, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  rejectMarker(categoryProjectionOwner, 'crate::category_presentation', 'retired Forum presentation owner');

  requireMarker(categoryCommandOwner, 'taxonomy_sync::move_category_in_tx', 'Taxonomy-owned Category move');
  requireMarker(categoryCommandOwner, 'taxonomy_sync::reorder_category_siblings_in_tx', 'Taxonomy-owned Category reorder');
  rejectMarker(categoryCommandOwner, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  requireMarker(categoryLifecycle, 'TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict', 'Taxonomy-owned lifecycle hierarchy read');
  rejectMarker(categoryLifecycle, 'rustok_taxonomy::entities', 'direct Taxonomy persistence access');
  rejectMarker(forumServices, 'include!("category_command.rs")', 'retired direct-persistence command include');

  if (fs.existsSync(categoryCommandLegacy)) failures.push(`${categoryCommandLegacy}: retired direct-persistence helper must be removed`);
  if (fs.existsSync(categoryPresentationLegacy)) failures.push(`${categoryPresentationLegacy}: retired Forum presentation owner must be removed`);

  requireMarker(forumLib, '["content", "media", "taxonomy"]', 'Forum runtime dependency set');
  requireMarker(forumManifest, 'taxonomy = { version_req = ">=0.1.0" }', 'Forum Taxonomy runtime dependency');
  rejectMarker(forumManifest, 'tenant = { version_req = ">=0.1.0" }', 'stale Forum tenant runtime dependency');
  rejectMarker(forumComposition, 'depends_on = ["content", "media", "taxonomy", "tenant"]', 'stale Forum tenant composition dependency');

  requireMarker(taxonomyHierarchy, 'pub async fn move_module_category_in_tx', 'Taxonomy Category move owner');
  requireMarker(taxonomyHierarchy, 'pub async fn shift_module_category_siblings_for_insert_in_tx', 'Taxonomy Category insertion owner');
  requireMarker(taxonomyRead, 'pub async fn load_module_category_sibling_ids_in', 'Taxonomy Category sibling read owner');
  requireMarker(taxonomyLib, 'move_module_category_in_tx', 'exported Taxonomy Category move owner');
  requireMarker(taxonomyLib, 'shift_module_category_siblings_for_insert_in_tx', 'exported Taxonomy Category insertion owner');
}

if (failures.length > 0) {
  console.error('[forum-taxonomy-category-binding] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[forum-taxonomy-category-binding] typed staged binding and Category ownership boundary verified');