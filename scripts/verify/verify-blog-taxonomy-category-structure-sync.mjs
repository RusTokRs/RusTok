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

const commandPath = 'crates/rustok-blog/src/services/category_command.rs';
const syncPath = 'crates/rustok-blog/src/services/category_taxonomy_sync.rs';
const libPath = 'crates/rustok-blog/src/lib.rs';
const evidencePath = 'crates/rustok-blog/src/translation_evidence.rs';
const runtimePath = 'crates/rustok-blog/tests/category_taxonomy_structure_sync.rs';

for (const path of [commandPath, syncPath, libPath, runtimePath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}
if (fs.existsSync(evidencePath)) {
  failures.push(`${evidencePath}: retired Category Translation evidence bridge must be absent`);
}

if (failures.length === 0) {
  const command = read(commandPath);
  const sync = read(syncPath);
  const lib = read(libPath);
  const runtime = read(runtimePath);

  requireMarker(command, 'touched.iter().copied().collect::<Vec<_>>()', 'bounded touched placement set');
  requireMarker(command, 'sync_category_structures_in_tx(&txn, tenant_id, &taxonomy_structure_ids).await?;', 'transactional move/reorder Taxonomy hook');
  requireMarker(command, 'txn.commit().await?;', 'Blog move transaction commit');

  requireMarker(sync, 'sync_module_category_structure_with_owned_copy_in_tx', 'Taxonomy-owned structure replay');
  requireMarker(sync, 'sync_siblings_for_parent_in_tx(txn, tenant_id, category.parent_id).await', 'create sibling compaction sync');
  requireMarker(sync, '.limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)', 'bounded sibling synchronization');
  requireMarker(sync, 'parent_id: category.parent_id', 'Blog placement donor parent');
  requireMarker(sync, 'position: category.position', 'Blog placement donor position');
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
