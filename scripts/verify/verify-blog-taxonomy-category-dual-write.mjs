#!/usr/bin/env node

import fs from 'node:fs';

const failures = [];
const read = (path) => fs.readFileSync(path, 'utf8');
const requireMarker = (source, marker, label = marker) => {
  if (!source.includes(marker)) failures.push(`missing ${label}`);
};
const rejectMarker = (source, marker, label = marker) => {
  if (source.includes(marker)) failures.push(`must not contain ${label}`);
};

const bridgePath = 'crates/rustok-blog/src/translation_evidence.rs';
const syncPath = 'crates/rustok-blog/src/services/category_taxonomy_sync.rs';
const servicesPath = 'crates/rustok-blog/src/services/mod.rs';
const runtimePath = 'crates/rustok-blog/tests/category_taxonomy_dual_write.rs';

for (const path of [bridgePath, syncPath, servicesPath, runtimePath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const bridge = read(bridgePath);
  const sync = read(syncPath);
  const services = read(servicesPath);
  const runtime = read(runtimePath);

  requireMarker(bridge, 'evidence.operation == "upsert"', 'active compatibility upsert gate');
  requireMarker(bridge, 'evidence.lifecycle == "active"', 'active compatibility lifecycle gate');
  requireMarker(bridge, 'blog_category_translation::Entity::find()', 'exact Blog compatibility copy reread');
  requireMarker(bridge, 'category_taxonomy_sync::sync_category_copy_in_tx', 'transactional Taxonomy copy hook');
  requireMarker(bridge, 'evidence.operation == "delete"', 'retired delete-journal transition');
  requireMarker(bridge, 'evidence.lifecycle == "deleted"', 'retired delete-journal lifecycle');
  requireMarker(bridge, 'Delete journal evidence is retired', 'explicit delete evidence retirement');
  rejectMarker(bridge, 'TranslationChangeActiveModel', 'retired Blog Translation change writer');
  rejectMarker(bridge, 'translation_change::', 'retired Blog Translation change entity dependency');
  rejectMarker(bridge, 'generate_id()', 'retired Blog Translation change id generation');
  rejectMarker(bridge, '.insert(transaction)', 'retired Blog Translation change insert');

  requireMarker(sync, 'sync_module_category_with_owned_aliases_in_tx', 'Taxonomy-owned route history sync');
  requireMarker(sync, 'module_scope: BLOG_TAXONOMY_SCOPE.to_string()', 'module/blog scope');
  requireMarker(sync, 'canonical_key_for_blog_category', 'deterministic Blog Category canonical key');
  requireMarker(sync, 'ensure_same_id_binding_in_tx', 'same-ID typed binding repair');
  requireMarker(sync, 'icon_key: None', 'no fabricated Taxonomy presentation');
  requireMarker(sync, 'color: None', 'no fabricated Taxonomy presentation');

  requireMarker(services, 'pub(crate) mod category_taxonomy_sync;', 'owner sync seam registration');
  requireMarker(runtime, 'category_create_and_update_dual_write_copy_routes_and_binding', 'create/update runtime proof');
  requireMarker(runtime, 'Blog compatibility mirror should remain during staged retirement', 'temporary mirror retention proof');
  requireMarker(runtime, 'retired Blog Category Translation evidence must not append change-journal rows', 'create journal retirement proof');
  requireMarker(runtime, 'canonical dual-write must no longer depend on the retired Blog change journal', 'update journal retirement proof');
  requireMarker(runtime, 'taxonomy_route_conflict_rolls_back_blog_create', 'transaction rollback proof');
  requireMarker(runtime, 'Taxonomy must own historical Blog route aliases', 'Taxonomy alias ownership proof');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-dual-write] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[blog-taxonomy-category-dual-write] canonical copy sync remains atomic without Blog Translation change-journal writes');
