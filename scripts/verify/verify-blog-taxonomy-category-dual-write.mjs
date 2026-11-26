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

const commandPath = 'crates/modules/rustok-blog/src/services/category/commands/copy.rs';
const ownerPath = 'crates/modules/rustok-blog/src/services/category/owner.rs';
const syncPath = 'crates/modules/rustok-blog/src/services/category/taxonomy_sync.rs';
const categoryPath = 'crates/modules/rustok-blog/src/services/category/mod.rs';
const libPath = 'crates/modules/rustok-blog/src/lib.rs';
const entitiesPath = 'crates/modules/rustok-blog/src/entities/mod.rs';
const bridgePath = 'crates/modules/rustok-blog/src/translation_evidence.rs';
const journalEntityPath = 'crates/modules/rustok-blog/src/entities/translation_change.rs';
const runtimePath = 'crates/modules/rustok-blog/tests/category_taxonomy_dual_write.rs';
const adapterPath = 'crates/modules/rustok-content-orchestration/src/blog_taxonomy.rs';

for (const path of [commandPath, ownerPath, syncPath, categoryPath, libPath, entitiesPath, runtimePath, adapterPath]) {
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
  const category = read(categoryPath);
  const lib = read(libPath);
  const entities = read(entitiesPath);
  const runtime = read(runtimePath);
  const adapter = read(adapterPath);

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
  rejectMarker(
    entities,
    'blog_category_translation',
    'retired donor translation entity must stay off the Blog entity surface',
  );

  requireMarker(sync, 'port.load_category_locale_copy_in_tx(', 'Blog owner locale read port');
  requireMarker(sync, 'port.sync_category_copy_in_tx(', 'Blog owner write port');
  requireMarker(adapter, 'load_module_category_locale_copy_in_tx(', 'Taxonomy owner locale read seam');
  requireMarker(adapter, 'sync_module_category_copy_in_tx(', 'Taxonomy-owned copy and route history sync');
  requireMarker(adapter, 'module_scope: BLOG_SCOPE.to_owned()', 'module/blog scope');
  requireMarker(adapter, 'format!("blog-category-{}", input.category_id)', 'deterministic Blog Category canonical key');
  const taxonomyOwnerSync = read('crates/modules/rustok-taxonomy/src/owner_category_sync.rs');
  requireMarker(taxonomyOwnerSync, 'ensure_category_identity(', 'same-ID Taxonomy identity guard');
  requireMarker(taxonomyOwnerSync, 'existing.tenant_id != tenant_id', 'tenant-bound Taxonomy identity guard');
  requireMarker(taxonomyOwnerSync, 'existing.canonical_key != canonical_key', 'canonical-key Taxonomy identity guard');
  requireMarker(taxonomyOwnerSync, 'include_owned_category_aliases_in_tx(txn, tenant_id, input)', 'Taxonomy-owned route history');
  requireMarker(taxonomyOwnerSync, 'CategoryPresentationSync::Keep', 'preservation of Taxonomy presentation');

  const host = read('apps/server/src/services/mod.rs');
  requireMarker(host, 'extensions.insert(rustok_content_orchestration::in_process_blog_taxonomy_port(', 'embedded host provider registration');
  requireMarker(host, 'rustok_content_orchestration::blog_category_delete_cleanup(', 'embedded host Category cleanup');
  for (const path of [
    'admin/src/transport/native_server_adapter.rs',
    'storefront/src/transport/native_server_adapter.rs',
    'storefront/src/transport/feed.rs',
  ]) {
    const source = read(`crates/modules/rustok-blog/${path}`);
    requireMarker(source, 'blog_taxonomy_from_runtime(', `host provider selection for ${path}`);
    requireMarker(source, '.with_taxonomy_port(', `service provider attachment for ${path}`);
  }

  requireMarker(category, 'mod taxonomy_sync;', 'owner sync seam registration');
  rejectMarker(category, '#[allow(dead_code)]', 'retired Category dead-code allowance');

  requireMarker(runtime, 'category_commands_use_taxonomy_after_legacy_storage_retirement', 'post-retirement command runtime proof');
  requireMarker(runtime, 'with_taxonomy_port(in_process_blog_taxonomy_port(', 'production port attached to test fixture');
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
