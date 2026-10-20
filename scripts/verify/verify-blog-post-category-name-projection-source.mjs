#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];

const files = {
  evidence: 'crates/modules/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json',
  postService: 'crates/modules/rustok-blog/src/services/post/queries.rs',
  categoryProjection: 'crates/modules/rustok-blog/src/services/category_name_projection.rs',
  dto: 'crates/modules/rustok-blog/src/dto/post.rs',
  harness: 'crates/modules/rustok-blog/tests/post_category_name_projection.rs',
};

function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!fs.existsSync(target)) {
    failures.push(`${relativePath}: missing file`);
    return '';
  }
  return fs.readFileSync(target, 'utf8');
}
function json(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}
function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}
function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}
function count(source, marker) {
  return source.split(marker).length - 1;
}

const evidence = json(files.evidence);
const postService = read(files.postService);
const categoryProjection = read(files.categoryProjection);
const dto = read(files.dto);
const harness = read(files.harness);

if (evidence) {
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== 'blog' ||
    evidence.surface !== 'post_category_name_projection' ||
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run' ||
    evidence.owner !== 'rustok-blog'
  ) failures.push(`${files.evidence}: identity/status drift`);

  const contract = evidence.source_contract ?? {};
  for (const [key, expected] of Object.entries({
    category_identity_source: 'blog_posts.category_id',
    category_name_source: 'taxonomy_owner_category.name',
    category_identity_path: 'blog_posts.category_id -> taxonomy_owner_category.id',
    taxonomy_scope: 'module/blog',
    tenant_bound_category_read: true,
    detail_projection_present: true,
    authenticated_list_projection_present: true,
    public_list_projection_present: true,
    list_projection_uses_batch_category_query: true,
    list_projection_avoids_per_post_category_query: true,
    category_ids_deduplicated_before_query: true,
    requested_locale_precedence: true,
    tenant_fallback_precedence: true,
    platform_fallback_precedence: true,
    first_available_fallback_retained: true,
    incomplete_projection_fails_closed: true,
    legacy_category_translation_read_removed: true,
    category_translation_write_semantics_changed: false,
    category_translation_readiness_promoted: false,
    search_projection_source_changed: false,
    database_schema_changed: false,
    graphql_schema_changed: false,
    http_schema_changed: false,
    ffa_promoted: false,
    fba_promoted: false,
  })) {
    if (contract[key] !== expected) failures.push(`${files.evidence}: ${key} drift`);
  }

  if (
    evidence.production_source !== files.postService ||
    evidence.category_projection_source !== files.categoryProjection ||
    evidence.dto_source !== files.dto ||
    evidence.source_harness?.path !== files.harness ||
    evidence.source_harness?.status !== 'executable_no_run' ||
    evidence.source_harness?.runtime_status !== 'not_run' ||
    evidence.planning_result?.post_category_name_source_complete !== true ||
    evidence.planning_result?.legacy_blog_category_translation_projection_retired !== true ||
    evidence.planning_result?.additional_category_name_projection_scaffolding_required !== false ||
    !Array.isArray(evidence.execution) ||
    evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: source/harness/planning drift`);
}

if (count(dto, 'pub category_name: Option<String>') < 2) {
  failures.push(`${files.dto}: detail/list category_name DTO contract missing`);
}

for (const marker of [
  'async fn load_category_names_map(',
  'crate::services::category_name_projection::load_category_names_map(',
  'let category_names_map = self',
  '.and_then(|category_id| category_names_map.get(&category_id).cloned())',
  'let category_name = if let Some(category_id) = post.category_id',
  'category_name,',
]) need(postService, marker, files.postService);
if (count(postService, '.load_category_names_map(') !== 3) {
  failures.push(`${files.postService}: expected exactly three category-name projection calls`);
}
forbid(postService, 'blog_category_translation', files.postService);
forbid(postService, 'category_name: None', files.postService);
forbid(postService, 'CategoryService::new(self.db.clone()', files.postService);

for (const marker of [
  'TaxonomyOwnerCategoryReader',
  'load_scoped_categories_strict',
  'TaxonomyScopeType::Module',
  'const BLOG_TAXONOMY_SCOPE: &str = "blog";',
  'Some(&category_ids)',
  'coverage is incomplete',
  'canonical.len() != category_ids.len()',
  'canonical_by_id.len() != category_ids.len()',
  'available_locales.is_empty()',
  'projection is missing',
  // The Taxonomy read must receive the caller's fallback locale: the argument
  // list is pinned so the parameter declaration cannot satisfy the guard.
  'Some(&category_ids),\n            locale,\n            fallback_locale,\n        )',
  'canonical.name.clone()',
]) need(categoryProjection, marker, files.categoryProjection);
forbid(categoryProjection, 'blog_category_translation', files.categoryProjection);
forbid(categoryProjection, 'CategoryService', files.categoryProjection);

for (const marker of [
  'post_category_name_projects_across_detail_and_list_paths',
  'CreateCategoryInput',
  'name: "Nachrichten".to_string()',
  'has_table("blog_category_translations")',
  'post Category projection contract must run after donor storage retirement',
  'Taxonomy category name through fallback',
  'get_post_with_locale_fallback(',
  'Some("de")',
  'list_posts_with_locale_fallback(',
  'list_public_visible_keyset(',
  'detail.category_name.as_deref()',
  'listed.items[0].category_name.as_deref()',
  'public.items[0].category_name.as_deref()',
]) need(harness, marker, files.harness);

if (failures.length) {
  console.error('[verify-blog-post-category-name-projection-source] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('[verify-blog-post-category-name-projection-source] PASS owner=taxonomy identity=same-id detail=list=public batch=true execution=not-run');
