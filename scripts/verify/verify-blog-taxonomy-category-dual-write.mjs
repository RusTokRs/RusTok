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

const commandPath = 'crates/rustok-blog/src/services/category.rs';
const ownerPath = 'crates/rustok-blog/src/services/category_owner.rs';
const syncPath = 'crates/rustok-blog/src/services/category_taxonomy_sync.rs';
const servicesPath = 'crates/rustok-blog/src/services/mod.rs';
const libPath = 'crates/rustok-blog/src/lib.rs';
const entitiesPath = 'crates/rustok-blog/src/entities/mod.rs';
const bridgePath = 'crates/rustok-blog/src/translation_evidence.rs';
const journalEntityPath = 'crates/rustok-blog/src/entities/translation_change.rs';
const runtimePath = 'crates/rustok-blog/tests/category_taxonomy_dual_write.rs';

for (const path of [commandPath, ownerPath, syncPath, servicesPath, libPath, entitiesPath, runtimePath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}
if (fs.existsSync(bridgePath)) failures.push(`${bridgePath}: retired bridge source must be absent`);
if (fs.existsSync(journalEntityPath)) {
  failures.push(`${journalEntityPath}: retired Blog Translation journal entity source must be absent`);
}

if (failures.length === 0) {
  const command = read(commandPath);
  const owner = read(ownerPath);
  const sync = read(syncPath);
  const services = read(servicesPath);
  const lib = read(libPath);
  const entities = read(entitiesPath);
  const runtime = read(runtimePath);

  requireMarker(command, 'load_category_locale_copy_in_tx', 'transactional canonical patch read');
  requireMarker(command, 'sync_category_copy_in_tx', 'transactional canonical copy write');
  requireMarker(command, 'The retired `blog_category_translation` table is not a command source or sink.', 'explicit mirror retirement boundary');
  rejectMarker(command, 'blog_category_translation::', 'legacy Blog Category translation entity dependency');
  rejectMarker(command, 'TranslationChangeEvidence', 'retired Blog Translation evidence dependency');
  rejectMarker(command, 'record_translation_change_in_tx', 'retired Blog Translation bridge call');
  rejectMarker(command, 'apply_exact_translation_in_tx', 'retired provider-era exact apply seam');

  requireMarker(owner, 'commands: CategoryCommandCore', 'canonical command core composition');
  rejectMarker(owner, 'ApplyExactCategoryTranslationInput', 'retired exact Translation input');
  rejectMarker(owner, 'CategoryTranslationApplyResult', 'retired exact Translation result');
  rejectMarker(owner, 'apply_exact_translation_in_tx', 'retired exact Translation owner seam');
  rejectMarker(owner, 'pub(crate) fn database', 'retired provider database seam');

  rejectMarker(lib, 'mod translation_evidence;', 'retired Category Translation bridge module registration');
  rejectMarker(entities, 'mod translation_change;', 'retired Blog Translation journal entity registration');
  rejectMarker(entities, 'BlogTranslationChange', 'retired Blog Translation journal entity export');
  requireMarker(
    entities,
    'pub(crate) mod blog_category_translation;',
    'crate-private donor translation entity retained only for historical backfill',
  );

  requireMarker(sync, 'load_module_category_locale_copy_in_tx', 'Taxonomy owner locale read seam');
  requireMarker(sync, 'sync_module_category_with_owned_aliases_in_tx', 'Taxonomy-owned route history sync');
  requireMarker(sync, 'module_scope: BLOG_TAXONOMY_SCOPE.to_string()', 'module/blog scope');
  requireMarker(sync, 'canonical_key_for_blog_category', 'deterministic Blog Category canonical key');
  requireMarker(sync, 'ensure_same_id_binding_in_tx', 'same-ID typed binding repair');
  requireMarker(sync, 'icon_key: None', 'no fabricated Taxonomy presentation');
  requireMarker(sync, 'color: None', 'no fabricated Taxonomy presentation');

  requireMarker(services, 'pub(crate) mod category_taxonomy_sync;', 'owner sync seam registration');
  rejectMarker(services, '#[allow(dead_code)]', 'retired Category dead-code allowance');

  requireMarker(runtime, 'category_commands_use_taxonomy_after_legacy_storage_retirement', 'post-retirement command runtime proof');
  requireMarker(runtime, '.has_table("blog_category_translations")', 'physical donor translation retirement proof');
  requireMarker(runtime, '.has_table("blog_translation_changes")', 'physical donor journal retirement proof');
  requireMarker(runtime, 'settings-only update must read canonical copy without donor tables', 'canonical patch source proof');
  requireMarker(runtime, 'taxonomy_route_conflict_rolls_back_blog_create', 'transaction rollback proof');
  requireMarker(runtime, 'Taxonomy must own historical Blog route aliases', 'Taxonomy alias ownership proof');
  rejectMarker(runtime, 'LEGACY POISON', 'obsolete poison fixture after physical storage retirement');
  rejectMarker(runtime, 'blog_category_translation::', 'retired donor entity runtime dependency');
  rejectMarker(runtime, 'translation_change::', 'retired donor journal runtime dependency');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-command-copy] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[blog-taxonomy-category-command-copy] canonical commands remain live with retired Blog Category legacy sources absent');
