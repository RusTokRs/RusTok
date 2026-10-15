#!/usr/bin/env node

// Successor contract for the Blog tag list pagination.
//
// The surface stayed "owner-bounded page size, no manual offset arithmetic", but the
// implementation now paginates in the database (`blog_tag_usage` paginator with a bounded
// page size, an explicit last-page guard, and canonical Taxonomy terms loaded for the page)
// instead of materializing a bounded tree and slicing it in Rust. The evidence file carries
// schema version 2 for that reason; this verifier asserts the v2 contract, the guarded
// database-side read, and the harnesses that execute both claims.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];
const files = {
  evidence: 'crates/modules/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  service: 'crates/modules/rustok-blog/src/services/tag.rs',
  dto: 'crates/modules/rustok-blog/src/dto/tag.rs',
  integrationHarness: 'crates/modules/rustok-blog/tests/taxonomy_tags.rs',
  slice: 'crates/modules/rustok-blog/docs/implementation-plan-slice-102.md',
  current: 'crates/modules/rustok-blog/docs/implementation-plan-current.md',
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

const evidence = json(files.evidence);
const service = read(files.service);
const dto = read(files.dto);
const integrationHarness = read(files.integrationHarness);
const slice = read(files.slice);
const current = read(files.current);

if (evidence) {
  if (
    evidence.schema_version !== 2 ||
    evidence.module !== 'blog' ||
    evidence.surface !== 'tag_list_pagination' ||
    evidence.owner !== 'rustok-blog' ||
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: identity/status drift`);

  const contract = evidence.source_contract ?? {};
  for (const [key, expected] of Object.entries({
    service: files.service,
    dto: files.dto,
    maximum_page_size: 100,
    minimum_page_size: 1,
    owner_service_clamps_page_size: true,
    dto_documents_minimum_page: true,
    dto_documents_page_size_bounds: true,
    page_index_uses_saturating_arithmetic: true,
    extreme_page_skips_database_fetch_after_total_count_guard: true,
    total_count_semantics_changed: false,
    sort_order_changed: false,
    tag_visibility_scope_changed: false,
    database_side_pagination_claimed: true,
    database_side_sorting: 'use_count_desc_canonical_key_asc_tag_id_asc',
    canonical_sort_key_source: 'taxonomy_terms.canonical_key_projected_into_blog_tag_usage',
    usage_projection_table: 'blog_tag_usage',
    global_tag_visibility_requires_positive_usage: true,
    module_blog_tags_remain_visible_at_zero_usage: true,
    usage_projection_is_transactionally_updated_with_blog_post_tags: true,
    post_deletion_decrements_usage_projection: true,
    taxonomy_tag_delete_removes_usage_projection_by_fk_cascade: true,
    migration_backfills_usage_projection_from_blog_post_tags: true,
    production_behavior_changed: true,
    database_schema_changed: true,
    ffa_promoted: false,
    fba_promoted: false,
  })) {
    if (contract[key] !== expected) failures.push(`${files.evidence}: ${key} drift`);
  }

  if (
    JSON.stringify(evidence.source_harness?.paths) !==
      JSON.stringify([files.service, files.integrationHarness]) ||
    JSON.stringify(evidence.source_harness?.tests) !==
      JSON.stringify([
        'services::tag::pagination_tests::tag_page_size_is_bounded_by_owner_service',
        'tag_list_is_bounded_by_database_pagination_and_preserves_zero_use_module_terms',
      ]) ||
    evidence.source_harness?.status !== 'executable_no_run' ||
    evidence.source_harness?.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: source harness drift`);

  if (
    evidence.open_follow_up?.tag_mutation_projection_consistency !==
      'retained_execution_in_tag_mutation_reindex_evidence' ||
    !evidence.open_follow_up?.reason?.includes('remaining work is maintainer execution') ||
    !Array.isArray(evidence.execution) ||
    evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: follow-up/execution drift`);
}

for (const marker of [
  'const MAX_TAGS_PER_PAGE: u64 = 100;',
  'let page = filter.page.max(1);',
  'let per_page = bounded_tag_page_size(filter.per_page);',
  'fn bounded_tag_page_size(value: u64) -> u64 {',
  'value.clamp(1, MAX_TAGS_PER_PAGE)',
  '.paginate(&txn, per_page);',
  'let total = paginator.num_items()',
  'let last_page = total.saturating_add(per_page.saturating_sub(1)) / per_page;',
  'paginator\n            .fetch_page(page - 1)',
  'mod pagination_tests',
  'fn tag_page_size_is_bounded_by_owner_service()',
  'bounded_tag_page_size(MAX_TAGS_PER_PAGE + 1)',
]) need(service, marker, files.service);

for (const marker of [
  '#[param(minimum = 1)]',
  '#[param(minimum = 1, maximum = 100)]',
]) need(dto, marker, files.dto);

for (const marker of [
  'let per_page = filter.per_page.max(1);',
  '((page - 1) * per_page) as usize',
]) forbid(service, marker, files.service);

need(
  integrationHarness,
  'async fn tag_list_is_bounded_by_database_pagination_and_preserves_zero_use_module_terms()',
  files.integrationHarness,
);

for (const marker of [
  'Status: `tag_list_pagination_owner_bound_source_ready_maintainer_execution_pending`.',
  '1 <= per_page <= 100',
  'saturating arithmetic',
  'does **not** claim database-side pagination',
  'TagService::update_tag/delete_tag',
  'No tests, Cargo commands, Node verifiers',
  'Re-audit Blog tag mutation/projection consistency',
]) need(slice, marker, files.slice);

for (const marker of [
  'tag_list_pagination = source_complete_maintainer_execution_pending',
  'tag_mutation_atomic_reindex = source_complete_maintainer_execution_pending',
]) need(current, marker, files.current);

if (failures.length > 0) {
  console.error('[verify-blog-tag-pagination-source] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '[verify-blog-tag-pagination-source] PASS max_per_page=100 pagination=database-side page_guard=out-of-range execution=not-run',
);
