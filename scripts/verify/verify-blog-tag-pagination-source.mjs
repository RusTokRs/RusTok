#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];
const files = {
  evidence: 'crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json',
  service: 'crates/rustok-blog/src/services/tag.rs',
  dto: 'crates/rustok-blog/src/dto/tag.rs',
  slice: 'crates/rustok-blog/docs/implementation-plan-slice-102.md',
  current: 'crates/rustok-blog/docs/implementation-plan-current.md',
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
const slice = read(files.slice);
const current = read(files.current);

if (evidence) {
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== 'blog' ||
    evidence.surface !== 'tag_list_pagination' ||
    evidence.owner !== 'rustok-blog' ||
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' ||
    evidence.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: identity/status drift`);

  const contract = evidence.source_contract ?? {};
  if (
    contract.service !== files.service ||
    contract.dto !== files.dto ||
    contract.maximum_page_size !== 100 ||
    contract.minimum_page_size !== 1 ||
    contract.owner_service_clamps_page_size !== true ||
    contract.dto_documents_minimum_page !== true ||
    contract.dto_documents_page_size_bounds !== true ||
    contract.page_offset_uses_saturating_arithmetic !== true ||
    contract.page_offset_uses_checked_usize_conversion !== true ||
    contract.extreme_page_cannot_overflow_offset_arithmetic !== true ||
    contract.total_count_semantics_changed !== false ||
    contract.sort_order_changed !== false ||
    contract.tag_visibility_scope_changed !== false ||
    contract.database_side_pagination_claimed !== false ||
    contract.production_behavior_changed !== true ||
    contract.database_schema_changed !== false ||
    contract.ffa_promoted !== false ||
    contract.fba_promoted !== false
  ) failures.push(`${files.evidence}: source contract drift`);

  if (
    evidence.source_harness?.path !== files.service ||
    evidence.source_harness?.module !== 'services::tag::pagination_tests' ||
    evidence.source_harness?.status !== 'executable_no_run' ||
    evidence.source_harness?.runtime_status !== 'not_run' ||
    JSON.stringify(evidence.source_harness?.tests) !== JSON.stringify([
      'tag_page_size_is_bounded_by_owner_service',
      'tag_page_offset_saturates_without_arithmetic_overflow',
    ])
  ) failures.push(`${files.evidence}: source harness drift`);

  if (
    evidence.open_follow_up?.tag_mutation_projection_consistency !== 're_audit_required' ||
    !evidence.open_follow_up?.reason?.includes('Search currently projects tag text') ||
    !Array.isArray(evidence.execution) ||
    evidence.execution.length !== 0
  ) failures.push(`${files.evidence}: follow-up/execution drift`);
}

for (const marker of [
  'const MAX_TAGS_PER_PAGE: u64 = 100;',
  'let page = filter.page.max(1);',
  'let per_page = bounded_tag_page_size(filter.per_page);',
  'let offset = tag_page_offset(page, per_page);',
  'fn bounded_tag_page_size(value: u64) -> u64 {',
  'value.clamp(1, MAX_TAGS_PER_PAGE)',
  'fn tag_page_offset(page: u64, per_page: u64) -> usize {',
  'page.saturating_sub(1).saturating_mul(per_page)',
  'usize::try_from(offset).unwrap_or(usize::MAX)',
  'mod pagination_tests',
  'fn tag_page_size_is_bounded_by_owner_service()',
  'fn tag_page_offset_saturates_without_arithmetic_overflow()',
  'tag_page_offset(u64::MAX, MAX_TAGS_PER_PAGE)',
]) need(service, marker, files.service);

for (const marker of [
  '#[param(minimum = 1)]',
  '#[param(minimum = 1, maximum = 100)]',
]) need(dto, marker, files.dto);

for (const marker of [
  'let per_page = filter.per_page.max(1);',
  '((page - 1) * per_page) as usize',
]) forbid(service, marker, files.service);

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
  'tag_list_pagination = source_ready_maintainer_execution_pending',
  'tag_mutation_projection_consistency = re_audit_required',
]) need(current, marker, files.current);

if (failures.length > 0) {
  console.error('[verify-blog-tag-pagination-source] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('[verify-blog-tag-pagination-source] PASS max_per_page=100 offset=overflow-safe execution=not-run');
