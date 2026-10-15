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

const ownerPath = 'crates/modules/rustok-blog/src/services/category_owner.rs';
const servicesPath = 'crates/modules/rustok-blog/src/services/mod.rs';
const legacyPath = 'crates/modules/rustok-blog/src/services/category.rs';
const runtimePath = 'crates/modules/rustok-blog/tests/category_taxonomy_read_cutover.rs';

for (const path of [ownerPath, servicesPath, legacyPath, runtimePath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const owner = read(ownerPath);
  const services = read(servicesPath);
  const legacy = read(legacyPath);
  const runtime = read(runtimePath);

  requireMarker(services, 'pub use category_owner::CategoryService;', 'public Category owner facade');
  rejectMarker(services, 'pub use category::CategoryService;', 'legacy CategoryService public re-export');

  requireMarker(owner, 'TaxonomyOwnerCategoryReader', 'Taxonomy owner projection');
  requireMarker(owner, 'load_scoped_categories_strict', 'canonical Taxonomy Category read');
  requireMarker(owner, 'Some(BLOG_TAXONOMY_SCOPE)', 'module/blog scope');
  requireMarker(owner, 'position: canonical.position', 'Taxonomy-owned sibling position');
  requireMarker(owner, 'parent_id,', 'Taxonomy-owned parent mapping');
  requireMarker(owner, 'settings: category.settings', 'Blog-owned settings composition');
  requireMarker(owner, '.limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)', 'bounded Blog membership read');
  requireMarker(owner, 'self.commands.create(tenant_id, security, input).await', 'mutation create remains delegated');
  requireMarker(owner, 'delete_module_category_with_cleanup(', 'delete lifecycle remains delegated to Taxonomy');
  rejectMarker(owner, 'blog_category_translation', 'legacy localized copy read in public owner facade');

  requireMarker(legacy, 'is not a command source or sink', 'retired donor storage boundary');

  requireMarker(runtime, 'has_table("blog_category_translations")', 'physical donor translation retirement proof');
  requireMarker(runtime, 'child_item.parent_id, Some(root)', 'Taxonomy-owned list placement proof');
  requireMarker(runtime, 'assert_eq!(read.effective_locale, "en")', 'requested/effective locale fallback proof');
  requireMarker(runtime, 'assert_eq!(read.parent_id, Some(root))', 'Taxonomy parent proof');
  requireMarker(runtime, '"layout": "blog-owned"', 'Blog settings preservation proof');
}

if (failures.length > 0) {
  console.error('[blog-taxonomy-category-read-cutover] boundary verification failed');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('[blog-taxonomy-category-read-cutover] public Category read ownership verified');
