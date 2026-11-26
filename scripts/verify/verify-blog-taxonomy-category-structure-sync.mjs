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

const commandPath = 'crates/modules/rustok-blog/src/services/category/commands/placement.rs';
const createPath = 'crates/modules/rustok-blog/src/services/category/commands/copy.rs';
const syncPath = 'crates/modules/rustok-blog/src/services/category/taxonomy_sync.rs';
const adapterPath = 'crates/modules/rustok-content-orchestration/src/blog_taxonomy.rs';
const libPath = 'crates/modules/rustok-blog/src/lib.rs';
const evidencePath = 'crates/modules/rustok-blog/src/translation_evidence.rs';
const runtimePath = 'crates/modules/rustok-blog/tests/category_taxonomy_structure_sync.rs';

for (const path of [commandPath, createPath, syncPath, adapterPath, libPath, runtimePath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}
if (fs.existsSync(evidencePath)) {
  failures.push(`${evidencePath}: retired Category Translation evidence bridge must be absent`);
}

if (failures.length === 0) {
  const command = read(commandPath);
  const create = read(createPath);
  const sync = read(syncPath);
  const adapter = read(adapterPath);
  const lib = read(libPath);
  const runtime = read(runtimePath);

  requireMarker(command, 'touched.insert(category_id);', 'bounded touched placement set');
  requireMarker(command, 'port.reorder_category_siblings_in_tx(', 'transactional Blog reorder port');
  requireMarker(command, '.move_category_in_tx(', 'transactional Blog move port');
  requireMarker(adapter, 'rustok_taxonomy::reorder_module_category_siblings_in_tx(', 'Taxonomy-owned reorder');
  requireMarker(adapter, 'rustok_taxonomy::move_module_category_in_tx(', 'Taxonomy-owned move');
  requireMarker(command, '.limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)', 'bounded sibling read');
  requireMarker(command, 'txn.commit().await?;', 'Blog move transaction commit');

  requireMarker(create, 'canonicalize_siblings_for_insert_in_tx(', 'create sibling compaction');
  requireMarker(create, 'port.reorder_category_siblings_in_tx(', 'create sibling compaction sync');
  requireMarker(create, 'sync_category_copy_in_tx(', 'create placement donor sync');
  requireMarker(sync, 'parent_id,', 'Blog placement donor parent');
  requireMarker(sync, 'position,', 'Blog placement donor position');
  rejectMarker(sync, 'category.settings', 'Blog-owned settings copied into Taxonomy');
  rejectMarker(sync, 'post_count', 'Blog-owned counters copied into Taxonomy');

  rejectMarker(lib, 'mod translation_evidence;', 'retired Category Translation evidence bridge registration');

  requireMarker(runtime, 'create_at_index_keeps_taxonomy_sibling_positions_dense', 'create insertion runtime proof');
  requireMarker(runtime, 'move_reparent_syncs_taxonomy_parent_and_both_sibling_sets', 'move/reparent runtime proof');
  requireMarker(runtime, 'target_shifted.position, 1', 'destination sibling order proof');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-structure-sync] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[blog-taxonomy-category-structure-sync] transactional hierarchy boundary verified with retired Translation evidence source absent');
