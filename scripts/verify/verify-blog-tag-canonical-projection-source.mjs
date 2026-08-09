#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];
const files = {
  evidence: 'crates/rustok-blog/contracts/evidence/blog-tag-canonical-projection-source.json',
  tagService: 'crates/rustok-blog/src/services/tag.rs',
  blogReadHarness: 'crates/rustok-blog/tests/taxonomy_tags.rs',
  projector: 'crates/rustok-search/src/blog_projector.rs',
  searchHarness: 'crates/rustok-search/tests/blog_projection_postgres_test.rs',
  searchEvidence: 'crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json',
  slice: 'crates/rustok-blog/docs/implementation-plan-slice-103.md',
  current: 'crates/rustok-blog/docs/implementation-plan-current.md',
};
function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!fs.existsSync(target)) { failures.push(`${relativePath}: missing file`); return ''; }
  return fs.readFileSync(target, 'utf8');
}
function json(relativePath) {
  try { return JSON.parse(read(relativePath)); }
  catch (error) { failures.push(`${relativePath}: invalid JSON: ${error.message}`); return null; }
}
function need(source, marker, label) { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); }
function forbid(source, marker, label) { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); }

const evidence = json(files.evidence);
const tagService = read(files.tagService);
const blogReadHarness = read(files.blogReadHarness);
const projector = read(files.projector);
const searchHarness = read(files.searchHarness);
const searchEvidence = json(files.searchEvidence);
const slice = read(files.slice);
const current = read(files.current);

if (evidence) {
  if (
    evidence.schema_version !== 1 || evidence.module !== 'blog' ||
    evidence.surface !== 'tag_canonical_read_search_projection' ||
    evidence.status !== 'source_verified_no_compile' || evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: identity/status drift`);
  if (
    evidence.ownership?.dictionary_owner !== 'rustok-taxonomy' ||
    evidence.ownership?.attachment_owner !== 'rustok-blog' ||
    evidence.ownership?.attachment_table !== 'blog_post_tags' ||
    evidence.ownership?.compatibility_mirror !== 'blog_posts.metadata.tags' ||
    evidence.ownership?.metadata_tags_is_canonical_read_source !== false ||
    evidence.ownership?.metadata_tags_is_search_projection_source !== false
  ) failures.push(`${files.evidence}: ownership drift`);
  if (
    evidence.source_contract?.blog_read_harness !== files.blogReadHarness ||
    evidence.source_contract?.requested_post_ids_receive_explicit_empty_tag_entries !== true ||
    evidence.source_contract?.empty_relation_set_falls_back_to_metadata !== false ||
    evidence.source_contract?.blog_read_harness_rejects_metadata_resurrection !== true ||
    evidence.source_contract?.search_requires_blog_post_tags !== true ||
    evidence.source_contract?.search_requires_taxonomy_terms !== true ||
    evidence.source_contract?.search_requires_taxonomy_term_translations !== true ||
    evidence.source_contract?.taxonomy_joins_are_tenant_constrained !== true ||
    evidence.source_contract?.stale_metadata_tags_are_ignored_by_search_harness !== true ||
    evidence.source_contract?.tag_mutation_semantics_changed !== false ||
    evidence.source_contract?.tag_mutation_atomic_reindex_implemented !== false ||
    !Array.isArray(evidence.execution) || evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: source/execution drift`);
  if (evidence.next_source_gap?.status !== 'tag_mutation_atomic_reindex_next') failures.push(`${files.evidence}: next source gap drift`);
}

for (const marker of [
  'let mut tags_by_post = post_ids', '.map(|post_id| (post_id, Vec::new()))',
  'if relations.is_empty() {', 'return Ok(tags_by_post);',
  'resolve_term_names(tenant_id, &term_ids, locale, fallback_locale)',
]) need(tagService, marker, files.tagService);
for (const marker of [
  'post_read_does_not_resurrect_metadata_tags_after_relations_are_removed',
  'stale-metadata-tag',
  'blog_post_tag::Entity::delete_many()',
  'assert!(post.tags.is_empty());',
]) need(blogReadHarness, marker, files.blogReadHarness);

for (const marker of [
  "to_regclass('blog_post_tags')", "to_regclass('taxonomy_terms')", "to_regclass('taxonomy_term_translations')",
  'FROM blog_post_tags relation', 'JOIN taxonomy_terms term',
  'LEFT JOIN taxonomy_term_translations localized', 'LEFT JOIN taxonomy_term_translations fallback',
  'term.tenant_id = p.tenant_id', 'localized.tenant_id = p.tenant_id', 'fallback.tenant_id = p.tenant_id',
  'COALESCE(localized.name, fallback.name, term.canonical_key)',
]) need(projector, marker, files.projector);
for (const marker of ["jsonb_array_elements_text", "p.metadata -> 'tags'"]) forbid(projector, marker, files.projector);

for (const marker of [
  'blog_reindex_projects_attached_taxonomy_tags_not_metadata_snapshot', 'stale-metadata-only',
  'CREATE TABLE taxonomy_terms', 'CREATE TABLE taxonomy_term_translations', 'CREATE TABLE blog_post_tags',
]) need(searchHarness, marker, files.searchHarness);
if (
  searchEvidence?.production_contract?.blog_tag_attachment_source !== 'blog_post_tags' ||
  searchEvidence?.production_contract?.legacy_metadata_tags_are_projection_source !== false ||
  !(searchEvidence?.cases ?? []).some((item) => item.name === 'canonical_taxonomy_tag_projection')
) failures.push(`${files.searchEvidence}: canonical projection evidence drift`);

for (const marker of [
  'Status: `tag_canonical_read_search_projection_source_ready_maintainer_execution_pending`.',
  '`blog_tag_read_source = blog_post_tags_plus_taxonomy`',
  '`blog_search_tag_projection = relation_taxonomy_source_ready_maintainer_execution_pending`',
  'This slice does **not** change `TagService::update_tag` or `TagService::delete_tag` transaction semantics.',
  'metadata-only rows', 'Implement `tag_mutation_atomic_reindex` as slice 104',
  'No tests, Cargo commands, Node verifiers',
]) need(slice, marker, files.slice);
for (const marker of [
  '`tag_canonical_projection = source_ready_maintainer_execution_pending`',
  '`tag_mutation_atomic_reindex = next_source_gap`',
]) need(current, marker, files.current);

if (failures.length) {
  console.error('[verify-blog-tag-canonical-projection-source] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log('[verify-blog-tag-canonical-projection-source] PASS source=blog_post_tags+taxonomy execution=not-run');
